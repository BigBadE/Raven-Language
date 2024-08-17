use crate::finalize_generics;
use crate::output::TypesChecker;
use parking_lot::Mutex;
use std::sync::Arc;
use syntax::async_util::NameResolver;
use syntax::errors::ParsingError;
use syntax::program::code::{FinalizedField, FinalizedMemberField};
use syntax::program::r#struct::{FinalizedStruct, UnfinalizedStruct};
use syntax::program::syntax::Syntax;

/// Verifies if a struct is valid
pub async fn verify_struct(
    _process_manager: &TypesChecker,
    structure: UnfinalizedStruct,
    resolver: &dyn NameResolver,
    syntax: &Arc<Mutex<Syntax>>,
) -> Result<FinalizedStruct, ParsingError> {
    let mut finalized_fields = Vec::default();
    for field in structure.fields {
        let field = field.await?;
        let field_type = field.field.field_type.finalize(syntax.clone()).await;
        finalized_fields.push(FinalizedMemberField {
            modifiers: field.modifiers,
            attributes: field.attributes,
            field: FinalizedField { field_type, name: field.field.name },
        })
    }

    let output = FinalizedStruct {
        generics: finalize_generics(syntax, resolver, &structure.generics).await?,
        fields: finalized_fields,
        data: structure.data,
    };

    return Ok(output);
}
