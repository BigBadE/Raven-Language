use chalk_ir::{
    interner::{HasInterner, Interner},
    TyKind,
};
use chalk_ir::{
    AdtId, AliasTy, AssocTypeId, CanonicalVarKind, CanonicalVarKinds, ConstData, Constraint,
    Constraints, FnDefId, Goals, InEnvironment, Lifetime, OpaqueTy, OpaqueTyId,
    ProgramClauseImplication, ProgramClauses, ProjectionTy, QuantifiedWhereClauses,
    SeparatorTraitRef, Substitution, TraitId, Ty, TyData, VariableKind, VariableKinds, Variances,
};
use chalk_ir::{
    GenericArg, GenericArgData, Goal, GoalData, LifetimeData, ProgramClause, ProgramClauseData,
    QuantifiedWhereClause, Variance,
};
use std::fmt;
use std::fmt::Debug;
use std::sync::Arc;

/// Contains a bunch of types and functions for Chalk to interact with the Raven types.
#[derive(Debug, Copy, Clone, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub struct ChalkIr;

impl Interner for ChalkIr {
    /// All of these are copied from ChalkIr from the Chalk repository.
    type InternedType = Arc<TyData<ChalkIr>>;
    type InternedLifetime = LifetimeData<ChalkIr>;
    type InternedConst = Arc<ConstData<ChalkIr>>;
    type InternedConcreteConst = u32;
    type InternedGenericArg = GenericArgData<ChalkIr>;
    type InternedGoal = Arc<GoalData<ChalkIr>>;
    type InternedGoals = Vec<Goal<ChalkIr>>;
    type InternedSubstitution = Vec<GenericArg<ChalkIr>>;
    type InternedProgramClause = ProgramClauseData<ChalkIr>;
    type InternedProgramClauses = Vec<ProgramClause<ChalkIr>>;
    type InternedQuantifiedWhereClauses = Vec<QuantifiedWhereClause<ChalkIr>>;
    type InternedVariableKinds = Vec<VariableKind<ChalkIr>>;
    type InternedCanonicalVarKinds = Vec<CanonicalVarKind<ChalkIr>>;
    type InternedConstraints = Vec<InEnvironment<Constraint<ChalkIr>>>;
    type InternedVariances = Vec<Variance>;
    type DefId = u32;
    type InternedAdtId = u32;
    type Identifier = String;
    type FnAbi = ();

    /// Unused debug functions.
    fn debug_adt_id(
        _type_kind_id: AdtId<ChalkIr>,
        _fmt: &mut fmt::Formatter<'_>,
    ) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_trait_id(
        _type_kind_id: TraitId<ChalkIr>,
        _fmt: &mut fmt::Formatter<'_>,
    ) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_assoc_type_id(
        _id: AssocTypeId<ChalkIr>,
        _fmt: &mut fmt::Formatter<'_>,
    ) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_opaque_ty_id(
        _id: OpaqueTyId<ChalkIr>,
        _fmt: &mut fmt::Formatter<'_>,
    ) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_fn_def_id(_id: FnDefId<Self>, _fmt: &mut fmt::Formatter<'_>) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_alias(
        _alias: &AliasTy<ChalkIr>,
        _fmt: &mut fmt::Formatter<'_>,
    ) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_projection_ty(
        _proj: &ProjectionTy<ChalkIr>,
        _fmt: &mut fmt::Formatter<'_>,
    ) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_opaque_ty(
        _opaque_ty: &OpaqueTy<ChalkIr>,
        _fmt: &mut fmt::Formatter<'_>,
    ) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_ty(_ty: &Ty<ChalkIr>, _fmt: &mut fmt::Formatter<'_>) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_lifetime(
        _lifetime: &Lifetime<ChalkIr>,
        _fmt: &mut fmt::Formatter<'_>,
    ) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_generic_arg(
        _generic_arg: &GenericArg<ChalkIr>,
        _fmt: &mut fmt::Formatter<'_>,
    ) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_variable_kinds(
        _variable_kinds: &VariableKinds<Self>,
        _fmt: &mut fmt::Formatter<'_>,
    ) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_variable_kinds_with_angles(
        _variable_kinds: &VariableKinds<Self>,
        _fmt: &mut fmt::Formatter<'_>,
    ) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_canonical_var_kinds(
        _canonical_var_kinds: &CanonicalVarKinds<Self>,
        _fmt: &mut fmt::Formatter<'_>,
    ) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_goal(_goal: &Goal<ChalkIr>, _fmt: &mut fmt::Formatter<'_>) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_goals(_goals: &Goals<ChalkIr>, _fmt: &mut fmt::Formatter<'_>) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_program_clause_implication(
        _pci: &ProgramClauseImplication<ChalkIr>,
        _fmt: &mut fmt::Formatter<'_>,
    ) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_program_clause(
        _clause: &ProgramClause<ChalkIr>,
        _fmt: &mut fmt::Formatter<'_>,
    ) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_program_clauses(
        _clause: &ProgramClauses<ChalkIr>,
        _fmt: &mut fmt::Formatter<'_>,
    ) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_substitution(
        _substitution: &Substitution<ChalkIr>,
        _fmt: &mut fmt::Formatter<'_>,
    ) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_separator_trait_ref(
        _separator_trait_ref: &SeparatorTraitRef<'_, ChalkIr>,
        _fmt: &mut fmt::Formatter<'_>,
    ) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_quantified_where_clauses(
        _clauses: &QuantifiedWhereClauses<Self>,
        _fmt: &mut fmt::Formatter<'_>,
    ) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_constraints(
        _constraints: &Constraints<Self>,
        _fmt: &mut fmt::Formatter<'_>,
    ) -> Option<fmt::Result> {
        unreachable!()
    }

    fn debug_variances(
        _variances: &Variances<Self>,
        _fmt: &mut fmt::Formatter<'_>,
    ) -> Option<fmt::Result> {
        unreachable!()
    }

    /// Copied from Chalk's interner.
    fn intern_ty(self, kind: TyKind<ChalkIr>) -> Arc<TyData<ChalkIr>> {
        let flags = kind.compute_flags(self);
        Arc::new(TyData { kind, flags })
    }

    /// Nothing is actually interned so the type is just returned
    fn ty_data(self, ty: &Arc<TyData<ChalkIr>>) -> &TyData<Self> {
        ty
    }

    fn intern_lifetime(self, lifetime: LifetimeData<ChalkIr>) -> LifetimeData<ChalkIr> {
        lifetime
    }

    fn lifetime_data(self, lifetime: &LifetimeData<ChalkIr>) -> &LifetimeData<ChalkIr> {
        lifetime
    }

    fn intern_const(self, constant: ConstData<ChalkIr>) -> Arc<ConstData<ChalkIr>> {
        Arc::new(constant)
    }

    fn const_data(self, constant: &Arc<ConstData<ChalkIr>>) -> &ConstData<ChalkIr> {
        constant
    }

    fn const_eq(self, _ty: &Arc<TyData<ChalkIr>>, c1: &u32, c2: &u32) -> bool {
        c1 == c2
    }

    fn intern_generic_arg(self, generic_arg: GenericArgData<ChalkIr>) -> GenericArgData<ChalkIr> {
        generic_arg
    }

    fn generic_arg_data(self, generic_arg: &GenericArgData<ChalkIr>) -> &GenericArgData<ChalkIr> {
        generic_arg
    }

    fn intern_goal(self, goal: GoalData<ChalkIr>) -> Arc<GoalData<ChalkIr>> {
        Arc::new(goal)
    }

    fn goal_data(self, goal: &Arc<GoalData<ChalkIr>>) -> &GoalData<ChalkIr> {
        goal
    }

    fn intern_goals<E>(
        self,
        data: impl IntoIterator<Item = Result<Goal<ChalkIr>, E>>,
    ) -> Result<Vec<Goal<ChalkIr>>, E> {
        data.into_iter().collect()
    }

    fn goals_data(self, goals: &Vec<Goal<ChalkIr>>) -> &[Goal<ChalkIr>] {
        goals
    }

    fn intern_substitution<E>(
        self,
        data: impl IntoIterator<Item = Result<GenericArg<ChalkIr>, E>>,
    ) -> Result<Vec<GenericArg<ChalkIr>>, E> {
        data.into_iter().collect()
    }

    fn substitution_data(self, substitution: &Vec<GenericArg<ChalkIr>>) -> &[GenericArg<ChalkIr>] {
        substitution
    }

    fn intern_program_clause(self, data: ProgramClauseData<Self>) -> ProgramClauseData<Self> {
        data
    }

    fn program_clause_data(self, clause: &ProgramClauseData<Self>) -> &ProgramClauseData<Self> {
        clause
    }

    fn intern_program_clauses<E>(
        self,
        data: impl IntoIterator<Item = Result<ProgramClause<Self>, E>>,
    ) -> Result<Vec<ProgramClause<Self>>, E> {
        data.into_iter().collect()
    }

    fn program_clauses_data(self, clauses: &Vec<ProgramClause<Self>>) -> &[ProgramClause<Self>] {
        clauses
    }

    fn intern_quantified_where_clauses<E>(
        self,
        data: impl IntoIterator<Item = Result<QuantifiedWhereClause<Self>, E>>,
    ) -> Result<Self::InternedQuantifiedWhereClauses, E> {
        data.into_iter().collect()
    }

    fn quantified_where_clauses_data(
        self,
        clauses: &Self::InternedQuantifiedWhereClauses,
    ) -> &[QuantifiedWhereClause<Self>] {
        clauses
    }

    fn intern_generic_arg_kinds<E>(
        self,
        data: impl IntoIterator<Item = Result<VariableKind<ChalkIr>, E>>,
    ) -> Result<Self::InternedVariableKinds, E> {
        data.into_iter().collect()
    }

    fn variable_kinds_data(
        self,
        variable_kinds: &Self::InternedVariableKinds,
    ) -> &[VariableKind<ChalkIr>] {
        variable_kinds
    }

    fn intern_canonical_var_kinds<E>(
        self,
        data: impl IntoIterator<Item = Result<CanonicalVarKind<ChalkIr>, E>>,
    ) -> Result<Self::InternedCanonicalVarKinds, E> {
        data.into_iter().collect()
    }

    fn canonical_var_kinds_data(
        self,
        canonical_var_kinds: &Self::InternedCanonicalVarKinds,
    ) -> &[CanonicalVarKind<ChalkIr>] {
        canonical_var_kinds
    }

    fn intern_constraints<E>(
        self,
        data: impl IntoIterator<Item = Result<InEnvironment<Constraint<Self>>, E>>,
    ) -> Result<Self::InternedConstraints, E> {
        data.into_iter().collect()
    }

    fn constraints_data(
        self,
        constraints: &Self::InternedConstraints,
    ) -> &[InEnvironment<Constraint<Self>>] {
        constraints
    }

    fn intern_variances<E>(
        self,
        data: impl IntoIterator<Item = Result<Variance, E>>,
    ) -> Result<Self::InternedVariances, E> {
        data.into_iter().collect()
    }

    fn variances_data(self, variances: &Self::InternedVariances) -> &[Variance] {
        variances
    }
}

impl HasInterner for ChalkIr {
    type Interner = ChalkIr;
}
