use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use chalk_ir::{AdtId, FnDefId, ImplId, ProgramClause, ProgramClauses, UnificationDatabase, Variance, Variances};
use chalk_solve::rust_ir::{AdtDatum, AdtRepr, AdtSizeAlign, AssociatedTyDatum,
                           AssociatedTyValue, AssociatedTyValueId, ClosureKind, FnDefDatum,
                           FnDefInputsAndOutputDatum, GeneratorDatum, GeneratorWitnessDatum,
                           ImplDatum, OpaqueTyDatum, TraitDatum, WellKnownTrait};
use chalk_solve::RustIrDatabase;
use crate::chalk_interner::{ChalkIr, RawId};
use crate::r#struct::ChalkData;
use crate::syntax::Syntax;

impl Debug for Syntax {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Syntax")
    }
}

impl RustIrDatabase<ChalkIr> for Syntax {
    fn custom_clauses(&self) -> Vec<ProgramClause<ChalkIr>> {
        return Vec::new();
    }

    fn associated_ty_data(&self, _ty: chalk_ir::AssocTypeId<ChalkIr>) -> Arc<AssociatedTyDatum<ChalkIr>> {
        todo!()
    }

    fn trait_datum(&self, trait_id: chalk_ir::TraitId<ChalkIr>) -> Arc<TraitDatum<ChalkIr>> {
        let found = self.structures.sorted.get(trait_id.0.index as usize - 1).unwrap();
        assert_eq!(found.id as u32, trait_id.0.index);
        if let ChalkData::Trait(_, _, inner) = found.chalk_data.as_ref().unwrap().clone() {
            return Arc::new(inner);
        }
        panic!("Expected a trait, got {:?}", found.name);
    }

    fn adt_datum(&self, adt_id: AdtId<ChalkIr>) -> Arc<AdtDatum<ChalkIr>> {
        let found = self.structures.sorted.get(adt_id.0.index as usize - 1).unwrap();
        assert_eq!(found.id as u32, adt_id.0.index);
        return Arc::new(found.chalk_data.as_ref().unwrap().get_adt().clone());
    }

    fn generator_datum(&self, _generator_id: chalk_ir::GeneratorId<ChalkIr>) -> Arc<GeneratorDatum<ChalkIr>> {
        todo!()
    }

    fn generator_witness_datum(&self, _generator_id: chalk_ir::GeneratorId<ChalkIr>) -> Arc<GeneratorWitnessDatum<ChalkIr>> {
        todo!()
    }

    fn adt_repr(&self, _id: AdtId<ChalkIr>) -> Arc<AdtRepr<ChalkIr>> {
        todo!()
    }

    fn adt_size_align(&self, _id: AdtId<ChalkIr>) -> Arc<AdtSizeAlign> {
        todo!()
    }

    fn fn_def_datum(&self, _fn_def_id: FnDefId<ChalkIr>) -> Arc<FnDefDatum<ChalkIr>> {
        todo!()
    }

    fn impl_datum(&self, impl_id: ImplId<ChalkIr>) -> Arc<ImplDatum<ChalkIr>> {
        return self.implementations.get(impl_id.0.index as usize).unwrap().chalk_type.clone();
    }

    fn associated_ty_value(&self, _id: AssociatedTyValueId<ChalkIr>) -> Arc<AssociatedTyValue<ChalkIr>> {
        todo!()
    }

    fn opaque_ty_data(&self, _id: chalk_ir::OpaqueTyId<ChalkIr>) -> Arc<OpaqueTyDatum<ChalkIr>> {
        todo!()
    }

    fn hidden_opaque_type(&self, _id: chalk_ir::OpaqueTyId<ChalkIr>) -> chalk_ir::Ty<ChalkIr> {
        todo!()
    }

    fn impls_for_trait(&self, trait_id: chalk_ir::TraitId<ChalkIr>, _parameters: &[chalk_ir::GenericArg<ChalkIr>],
                       _binders: &chalk_ir::CanonicalVarKinds<ChalkIr>) -> Vec<ImplId<ChalkIr>> {
        let mut output = Vec::new();
        let mut i = 0;
        for implementation in &self.implementations {
            if implementation.target.inner_struct().data.id as u32 == trait_id.0.index {
                output.push(ImplId(RawId {
                    index: i
                }));
            }
            i += 1;
        }
        output
    }

    fn local_impls_to_coherence_check(&self, _trait_id: chalk_ir::TraitId<ChalkIr>) -> Vec<ImplId<ChalkIr>> {
        todo!()
    }

    fn impl_provided_for(&self, _auto_trait_id: chalk_ir::TraitId<ChalkIr>, _ty: &chalk_ir::TyKind<ChalkIr>) -> bool {
        todo!()
    }

    fn well_known_trait_id(&self, _well_known_trait: WellKnownTrait) -> Option<chalk_ir::TraitId<ChalkIr>> {
        todo!()
    }

    fn program_clauses_for_env(&self, environment: &chalk_ir::Environment<ChalkIr>) -> ProgramClauses<ChalkIr> {
        chalk_solve::program_clauses_for_env(self, environment)
    }

    fn interner(&self) -> ChalkIr {
        ChalkIr
    }

    fn is_object_safe(&self, _trait_id: chalk_ir::TraitId<ChalkIr>) -> bool {
        todo!()
    }

    fn closure_kind(&self, _closure_id: chalk_ir::ClosureId<ChalkIr>, _substs: &chalk_ir::Substitution<ChalkIr>) -> ClosureKind {
        todo!()
    }

    fn closure_inputs_and_output(&self, _closure_id: chalk_ir::ClosureId<ChalkIr>, _substs: &chalk_ir::Substitution<ChalkIr>) -> chalk_ir::Binders<FnDefInputsAndOutputDatum<ChalkIr>> {
        todo!()
    }

    fn closure_upvars(&self, _closure_id: chalk_ir::ClosureId<ChalkIr>, _substs: &chalk_ir::Substitution<ChalkIr>) -> chalk_ir::Binders<chalk_ir::Ty<ChalkIr>> {
        todo!()
    }

    fn closure_fn_substitution(&self, _closure_id: chalk_ir::ClosureId<ChalkIr>, _substs: &chalk_ir::Substitution<ChalkIr>) -> chalk_ir::Substitution<ChalkIr> {
        todo!()
    }

    fn unification_database(&self) -> &dyn UnificationDatabase<ChalkIr> {
        self
    }

    fn discriminant_type(&self, _ty: chalk_ir::Ty<ChalkIr>) -> chalk_ir::Ty<ChalkIr> {
        todo!()
    }
}

impl UnificationDatabase<ChalkIr> for Syntax {
    fn fn_def_variance(&self, _fn_def_id: FnDefId<ChalkIr>) -> Variances<ChalkIr> {
        /*Variances::from_iter(
            self.interner(),
            self.fn_def_variances[&fn_def_id].iter().copied(),
        )*/
        todo!()
    }

    fn adt_variance(&self, _adt_id: AdtId<ChalkIr>) -> Variances<ChalkIr> {
        let variances: [Variance; 0] = [];
        Variances::from_iter(ChalkIr, /*self.adt_variances[&adt_id].iter().copied()*/ variances)
    }
}