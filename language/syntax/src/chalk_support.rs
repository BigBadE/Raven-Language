use crate::chalk_interner::ChalkIr;
use crate::r#struct::ChalkData;
use crate::syntax::Syntax;
use chalk_ir::{
    AdtId, FnDefId, ImplId, ProgramClause, ProgramClauses, UnificationDatabase, Variances,
};
use chalk_solve::rust_ir::{
    AdtDatum, AdtRepr, AdtSizeAlign, AssociatedTyDatum, AssociatedTyValue, AssociatedTyValueId,
    ClosureKind, FnDefDatum, FnDefInputsAndOutputDatum, GeneratorDatum, GeneratorWitnessDatum,
    ImplDatum, OpaqueTyDatum, TraitDatum, WellKnownTrait,
};
use chalk_solve::RustIrDatabase;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

impl Debug for Syntax {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Syntax")
    }
}

/// This implementation allows Chalk to interact with the Raven syntax.
impl RustIrDatabase<ChalkIr> for Syntax {
    /// Most of these are features not used by Raven, so they're empty.
    fn custom_clauses(&self) -> Vec<ProgramClause<ChalkIr>> {
        return Vec::default();
    }

    fn associated_ty_data(
        &self,
        _ty: chalk_ir::AssocTypeId<ChalkIr>,
    ) -> Arc<AssociatedTyDatum<ChalkIr>> {
        unreachable!()
    }

    /// Gets the trait given the ID.
    fn trait_datum(&self, trait_id: chalk_ir::TraitId<ChalkIr>) -> Arc<TraitDatum<ChalkIr>> {
        let found = self.structures.sorted.get(trait_id.0 as usize).unwrap();
        assert_eq!(found.id as u32, trait_id.0);
        if let ChalkData::Trait(_, _, inner) = found.chalk_data.as_ref().unwrap().clone() {
            return Arc::new(inner);
        }
        panic!("Expected a trait, got {:?}", found.name);
    }

    /// Gets the structure given the ID.
    fn adt_datum(&self, adt_id: AdtId<ChalkIr>) -> Arc<AdtDatum<ChalkIr>> {
        let found = self.structures.sorted.get(adt_id.0 as usize).unwrap();
        assert_eq!(found.id as u32, adt_id.0);
        return Arc::new(found.chalk_data.as_ref().unwrap().get_adt().clone());
    }

    fn generator_datum(
        &self,
        _generator_id: chalk_ir::GeneratorId<ChalkIr>,
    ) -> Arc<GeneratorDatum<ChalkIr>> {
        unreachable!()
    }

    fn generator_witness_datum(
        &self,
        _generator_id: chalk_ir::GeneratorId<ChalkIr>,
    ) -> Arc<GeneratorWitnessDatum<ChalkIr>> {
        unreachable!()
    }

    fn adt_repr(&self, _id: AdtId<ChalkIr>) -> Arc<AdtRepr<ChalkIr>> {
        unreachable!()
    }

    fn adt_size_align(&self, _id: AdtId<ChalkIr>) -> Arc<AdtSizeAlign> {
        unreachable!()
    }

    fn fn_def_datum(&self, _fn_def_id: FnDefId<ChalkIr>) -> Arc<FnDefDatum<ChalkIr>> {
        unreachable!()
    }

    /// Gets an implementation given the ID.
    fn impl_datum(&self, impl_id: ImplId<ChalkIr>) -> Arc<ImplDatum<ChalkIr>> {
        return self
            .implementations
            .get(impl_id.0 as usize)
            .unwrap()
            .chalk_type
            .clone();
    }

    fn associated_ty_value(
        &self,
        _id: AssociatedTyValueId<ChalkIr>,
    ) -> Arc<AssociatedTyValue<ChalkIr>> {
        unreachable!()
    }

    fn opaque_ty_data(&self, _id: chalk_ir::OpaqueTyId<ChalkIr>) -> Arc<OpaqueTyDatum<ChalkIr>> {
        unreachable!()
    }

    fn hidden_opaque_type(&self, _id: chalk_ir::OpaqueTyId<ChalkIr>) -> chalk_ir::Ty<ChalkIr> {
        unreachable!()
    }

    /// Finds all the implementations of a specific trait.
    fn impls_for_trait(
        &self,
        trait_id: chalk_ir::TraitId<ChalkIr>,
        _parameters: &[chalk_ir::GenericArg<ChalkIr>],
        _binders: &chalk_ir::CanonicalVarKinds<ChalkIr>,
    ) -> Vec<ImplId<ChalkIr>> {
        let mut output = Vec::default();
        let mut i = 0;
        for implementation in &self.implementations {
            if implementation.target.inner_struct().data.id as u32 == trait_id.0 {
                output.push(ImplId(i));
            }
            i += 1;
        }
        output
    }

    fn local_impls_to_coherence_check(
        &self,
        _trait_id: chalk_ir::TraitId<ChalkIr>,
    ) -> Vec<ImplId<ChalkIr>> {
        unreachable!()
    }

    fn impl_provided_for(
        &self,
        _auto_trait_id: chalk_ir::TraitId<ChalkIr>,
        _ty: &chalk_ir::TyKind<ChalkIr>,
    ) -> bool {
        unreachable!()
    }

    fn well_known_trait_id(
        &self,
        _well_known_trait: WellKnownTrait,
    ) -> Option<chalk_ir::TraitId<ChalkIr>> {
        unreachable!()
    }

    /// Copied from Chalk
    fn program_clauses_for_env(
        &self,
        environment: &chalk_ir::Environment<ChalkIr>,
    ) -> ProgramClauses<ChalkIr> {
        chalk_solve::program_clauses_for_env(self, environment)
    }

    fn interner(&self) -> ChalkIr {
        ChalkIr
    }

    fn is_object_safe(&self, _trait_id: chalk_ir::TraitId<ChalkIr>) -> bool {
        unreachable!()
    }

    fn closure_kind(
        &self,
        _closure_id: chalk_ir::ClosureId<ChalkIr>,
        _substs: &chalk_ir::Substitution<ChalkIr>,
    ) -> ClosureKind {
        unreachable!()
    }

    fn closure_inputs_and_output(
        &self,
        _closure_id: chalk_ir::ClosureId<ChalkIr>,
        _substs: &chalk_ir::Substitution<ChalkIr>,
    ) -> chalk_ir::Binders<FnDefInputsAndOutputDatum<ChalkIr>> {
        unreachable!()
    }

    fn closure_upvars(
        &self,
        _closure_id: chalk_ir::ClosureId<ChalkIr>,
        _substs: &chalk_ir::Substitution<ChalkIr>,
    ) -> chalk_ir::Binders<chalk_ir::Ty<ChalkIr>> {
        unreachable!()
    }

    fn closure_fn_substitution(
        &self,
        _closure_id: chalk_ir::ClosureId<ChalkIr>,
        _substs: &chalk_ir::Substitution<ChalkIr>,
    ) -> chalk_ir::Substitution<ChalkIr> {
        unreachable!()
    }

    fn unification_database(&self) -> &dyn UnificationDatabase<ChalkIr> {
        self
    }

    fn discriminant_type(&self, _ty: chalk_ir::Ty<ChalkIr>) -> chalk_ir::Ty<ChalkIr> {
        unreachable!()
    }
}

/// This all isn't used by Raven.
impl UnificationDatabase<ChalkIr> for Syntax {
    fn fn_def_variance(&self, _fn_def_id: FnDefId<ChalkIr>) -> Variances<ChalkIr> {
        unreachable!()
    }

    fn adt_variance(&self, _adt_id: AdtId<ChalkIr>) -> Variances<ChalkIr> {
        Variances::from_iter(ChalkIr, [])
    }
}
