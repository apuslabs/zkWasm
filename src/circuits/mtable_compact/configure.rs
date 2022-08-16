use super::*;
use crate::circuits::utils::bn_to_field;
use crate::circuits::Lookup;
use crate::constant;
use crate::constant_from;
use crate::curr;
use crate::fixed_curr;
use crate::nextn;
use halo2_proofs::arithmetic::FieldExt;
use halo2_proofs::plonk::Advice;
use halo2_proofs::plonk::Column;
use halo2_proofs::plonk::ConstraintSystem;
use specs::mtable::AccessType;
use specs::mtable::LocationType;

pub const STEP_SIZE: i32 = 8;

pub trait MemoryTableConstriants<F: FieldExt> {
    fn configure(
        &self,
        meta: &mut ConstraintSystem<F>,
        rtable: &RangeTableConfig<F>,
        imtable: &InitMemoryTableConfig<F>,
    ) {
        self.configure_enable_as_bit(meta, rtable);
        self.configure_rest_mops_decrease(meta, rtable);
        self.configure_final_rest_mops_zero(meta, rtable);
        self.configure_read_nochange(meta, rtable);
        self.configure_heap_first_init(meta, rtable);
        self.configure_stack_first_not_read(meta, rtable);
        self.configure_init_on_first(meta, rtable);
        self.configure_index_sort(meta, rtable);
        self.configure_heap_init_in_imtable(meta, rtable, imtable);
        self.configure_tvalue_bytes(meta, rtable);
        self.configure_encode_range(meta, rtable);
    }

    fn configure_enable_as_bit(&self, meta: &mut ConstraintSystem<F>, rtable: &RangeTableConfig<F>);
    fn configure_encode_range(&self, meta: &mut ConstraintSystem<F>, rtable: &RangeTableConfig<F>);
    fn configure_enable_seq(&self, meta: &mut ConstraintSystem<F>, rtable: &RangeTableConfig<F>);
    fn configure_index_sort(&self, meta: &mut ConstraintSystem<F>, rtable: &RangeTableConfig<F>);
    fn configure_rest_mops_decrease(
        &self,
        meta: &mut ConstraintSystem<F>,
        rtable: &RangeTableConfig<F>,
    );
    fn configure_final_rest_mops_zero(
        &self,
        meta: &mut ConstraintSystem<F>,
        rtable: &RangeTableConfig<F>,
    );
    fn configure_read_nochange(&self, meta: &mut ConstraintSystem<F>, rtable: &RangeTableConfig<F>);
    fn configure_heap_first_init(
        &self,
        meta: &mut ConstraintSystem<F>,
        rtable: &RangeTableConfig<F>,
    );
    fn configure_stack_first_not_read(
        &self,
        meta: &mut ConstraintSystem<F>,
        rtable: &RangeTableConfig<F>,
    );
    fn configure_init_on_first(&self, meta: &mut ConstraintSystem<F>, rtable: &RangeTableConfig<F>);
    fn configure_tvalue_bytes(&self, meta: &mut ConstraintSystem<F>, rtable: &RangeTableConfig<F>);
    fn configure_heap_init_in_imtable(
        &self,
        meta: &mut ConstraintSystem<F>,
        rtable: &RangeTableConfig<F>,
        imtable: &InitMemoryTableConfig<F>,
    );
}

impl<F: FieldExt> MemoryTableConstriants<F> for MemoryTableConfig<F> {
    fn configure_encode_range(&self, meta: &mut ConstraintSystem<F>, rtable: &RangeTableConfig<F>) {
        rtable.configure_in_common_range(meta, "mtable encode in common range", |meta| {
            curr!(meta, self.aux) * self.is_enabled_line(meta)
        })
    }

    fn configure_enable_as_bit(
        &self,
        meta: &mut ConstraintSystem<F>,
        _rtable: &RangeTableConfig<F>,
    ) {
        meta.create_gate("mtable configure_enable_as_bit", |meta| {
            vec![
                curr!(meta, self.bit)
                    * (curr!(meta, self.bit) - constant_from!(1))
                    * fixed_curr!(meta, self.sel),
            ]
        });
    }

    fn configure_enable_seq(&self, meta: &mut ConstraintSystem<F>, _rtable: &RangeTableConfig<F>) {
        meta.create_gate("mtable configure_enable_seq", |meta| {
            vec![
                nextn!(meta, self.bit, STEP_SIZE)
                    * (curr!(meta, self.bit) - constant_from!(1))
                    * fixed_curr!(meta, self.sel)
                    * fixed_curr!(meta, self.block_first_line_sel),
            ]
        });
    }

    fn configure_index_sort(&self, meta: &mut ConstraintSystem<F>, rtable: &RangeTableConfig<F>) {
        meta.create_gate("mtable configure_index_same", |meta| {
            vec![
                curr!(meta, self.aux) - constant_from!(1),
                self.same_mmid(meta) - self.same_ltype(meta) * self.same_mmid_single(meta),
                self.same_offset(meta) - self.same_mmid(meta) * self.same_offset_single(meta),
                self.same_eid(meta) - self.same_offset(meta) * self.same_eid_single(meta),
            ]
            .into_iter()
            .map(|e| e * self.is_enabled_following_block(meta))
            .collect::<Vec<_>>()
        });

        rtable.configure_in_common_range(meta, "mtable configure_index_sort", |meta| {
            (curr!(meta, self.index.data) - nextn!(meta, self.index.data, -STEP_SIZE))
                * curr!(meta, self.aux)
                * self.is_enabled_following_block(meta)
        });

        rtable.configure_in_common_range(meta, "mtable configure_index_sort", |meta| {
            curr!(meta, self.index.data) * self.is_enabled_line(meta)
        });
    }

    fn configure_rest_mops_decrease(
        &self,
        meta: &mut ConstraintSystem<F>,
        _rtable: &RangeTableConfig<F>,
    ) {
        meta.create_gate(
            "mtable configure_rest_mops_decrease decrease on non-init",
            |meta| {
                vec![
                    (self.prev_rest_mops(meta) - self.rest_mops(meta) - constant_from!(1))
                        * (self.prev_atype(meta) - constant_from!(AccessType::Init)),
                ]
                .into_iter()
                .map(|e| e * self.is_enabled_following_block(meta))
                .collect::<Vec<_>>()
            },
        );

        meta.create_gate(
            "mtable configure_rest_mops_decrease no decrease on init",
            |meta| {
                vec![
                    (self.prev_rest_mops(meta) - self.rest_mops(meta))
                        * (self.prev_atype(meta) - constant_from!(AccessType::Write))
                        * (self.prev_atype(meta) - constant_from!(AccessType::Read)),
                ]
                .into_iter()
                .map(|e| e * self.is_enabled_following_block(meta))
                .collect::<Vec<_>>()
            },
        );
    }

    fn configure_final_rest_mops_zero(
        &self,
        meta: &mut ConstraintSystem<F>,
        _rtable: &RangeTableConfig<F>,
    ) {
        meta.create_gate("mtable configure_final_rest_mops_zero", |meta| {
            vec![self.rest_mops(meta) * (curr!(meta, self.bit) - constant_from!(1))]
                .into_iter()
                .map(|e| e * self.is_enabled_following_block(meta))
                .collect::<Vec<_>>()
        });
    }

    fn configure_read_nochange(
        &self,
        meta: &mut ConstraintSystem<F>,
        _rtable: &RangeTableConfig<F>,
    ) {
        meta.create_gate("mtable configure_read_nochange value", |meta| {
            vec![
                (self.atype(meta) - constant_from!(AccessType::Write))
                    * (self.atype(meta) - constant_from!(AccessType::Init))
                    * self.same_offset(meta)
                    * (self.prev_value(meta) - self.value(meta)),
            ]
            .into_iter()
            .map(|e| e * self.is_enabled_following_block(meta))
            .collect::<Vec<_>>()
        });

        meta.create_gate("mtable configure_read_nochange vtype", |meta| {
            vec![
                (self.atype(meta) - constant_from!(AccessType::Write))
                    * (self.atype(meta) - constant_from!(AccessType::Init))
                    * self.same_offset(meta)
                    * (self.prev_vtype(meta) - self.vtype(meta)),
            ]
            .into_iter()
            .map(|e| e * self.is_enabled_following_block(meta))
            .collect::<Vec<_>>()
        });
    }

    fn configure_heap_first_init(
        &self,
        meta: &mut ConstraintSystem<F>,
        _rtable: &RangeTableConfig<F>,
    ) {
        meta.create_gate("mtable configure_heap_first_init", |meta| {
            vec![
                (self.ltype(meta) - constant_from!(LocationType::Stack))
                    * (constant_from!(1) - self.same_offset(meta))
                    * (self.atype(meta) - constant_from!(AccessType::Init)),
            ]
            .into_iter()
            .map(|e| e * self.is_enabled_following_block(meta))
            .collect::<Vec<_>>()
        });
    }

    fn configure_stack_first_not_read(
        &self,
        meta: &mut ConstraintSystem<F>,
        _rtable: &RangeTableConfig<F>,
    ) {
        meta.create_gate("mtable configure_stack_first_write", |meta| {
            vec![
                (self.ltype(meta) - constant_from!(LocationType::Heap))
                    * (constant_from!(1) - self.same_offset(meta))
                    * (self.atype(meta) - constant_from!(AccessType::Write))
                    * (self.atype(meta) - constant_from!(AccessType::Init)),
            ]
            .into_iter()
            .map(|e| e * self.is_enabled_following_block(meta))
            .collect::<Vec<_>>()
        });
    }

    fn configure_init_on_first(
        &self,
        meta: &mut ConstraintSystem<F>,
        _rtable: &RangeTableConfig<F>,
    ) {
        meta.create_gate("mtable configure_init_on_first", |meta| {
            vec![
                self.same_offset(meta)
                    * (self.atype(meta) - constant_from!(AccessType::Write))
                    * (self.atype(meta) - constant_from!(AccessType::Read)),
            ]
            .into_iter()
            .map(|e| e * self.is_enabled_following_block(meta))
            .collect::<Vec<_>>()
        });
    }

    fn configure_tvalue_bytes(&self, meta: &mut ConstraintSystem<F>, rtable: &RangeTableConfig<F>) {
        rtable.configure_in_u8_range(meta, "mtable bytes", |meta| {
            curr!(meta, self.bytes) * self.is_enabled_line(meta)
        });

        meta.create_gate("mtable byte mask consistent", |meta| {
            vec![
                (self.ge_two_bytes(meta) - constant_from!(1)) * self.byte(meta, 1),
                (self.ge_four_bytes(meta) - constant_from!(1))
                    * (self.byte(meta, 2) + self.byte(meta, 3)),
                (self.ge_eight_bytes(meta) - constant_from!(1))
                    * (self.byte(meta, 4)
                        + self.byte(meta, 5)
                        + self.byte(meta, 6)
                        + self.byte(meta, 7)),
                self.ge_eight_bytes(meta) * (self.ge_four_bytes(meta) - constant_from!(1)),
                self.ge_four_bytes(meta) * (self.ge_two_bytes(meta) - constant_from!(1)),
            ]
            .into_iter()
            .map(|e| e * self.is_enabled_following_block(meta))
            .collect::<Vec<_>>()
        });
    }

    fn configure_heap_init_in_imtable(
        &self,
        meta: &mut ConstraintSystem<F>,
        _rtable: &RangeTableConfig<F>,
        imtable: &InitMemoryTableConfig<F>,
    ) {
        imtable.configure_in_table(meta, "mtable configure_heap_init_in_imtable", |meta| {
            self.same_offset(meta)
                * self.is_heap(meta)
                * imtable.encode(self.mmid(meta), self.offset(meta), self.value(meta))
                * self.is_enabled_block(meta)
        })
    }
}

impl<F: FieldExt> Lookup<F> for MemoryTableConfig<F> {
    fn encode(
        &self,
        meta: &mut halo2_proofs::plonk::VirtualCells<'_, F>,
    ) -> halo2_proofs::plonk::Expression<F> {
        (self.eid(meta) * constant!(bn_to_field(&EID_SHIFT))
            + self.emid(meta) * constant!(bn_to_field(&EMID_SHIFT))
            + self.mmid(meta) * constant!(bn_to_field(&MMID_SHIFT))
            + self.offset(meta) * constant!(bn_to_field(&OFFSET_SHIFT))
            + self.ltype(meta) * constant!(bn_to_field(&LOC_TYPE_SHIFT))
            + self.atype(meta) * constant!(bn_to_field(&ACCESS_TYPE_SHIFT))
            + self.vtype(meta) * constant!(bn_to_field(&VAR_TYPE_SHIFT))
            + self.value(meta))
            * self.is_enabled_block(meta)
    }
}

impl<F: FieldExt> MemoryTableConfig<F> {
    pub(super) fn new(
        meta: &mut ConstraintSystem<F>,
        cols: &mut impl Iterator<Item = Column<Advice>>,
    ) -> Self {
        let sel = meta.fixed_column();
        let following_block_sel = meta.fixed_column();
        let block_first_line_sel = meta.fixed_column();
        let bit = cols.next().unwrap();
        let index = RowDiffConfig::configure("mtable index", meta, cols, STEP_SIZE, |meta| {
            fixed_curr!(meta, following_block_sel)
        });
        let aux = cols.next().unwrap();
        let bytes = cols.next().unwrap();

        MemoryTableConfig {
            sel,
            following_block_sel,
            block_first_line_sel,
            bit,
            index,
            aux,
            bytes,
        }
    }
}
