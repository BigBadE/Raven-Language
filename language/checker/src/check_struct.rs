use crate::finalize_generics;
use crate::output::TypesChecker;
use std::sync::Arc;
use std::sync::Mutex;
use syntax::errors::ParsingError;
use syntax::program::code::{FinalizedField, FinalizedMemberField};
use syntax::program::r#struct::{FinalizedStruct, UnfinalizedStruct};
use syntax::program::syntax::Syntax;
use syntax::program::types::FinalizedTypes;

/// Verifies if a struct is valid
pub async fn verify_struct(
    _process_manager: &TypesChecker,
    structure: UnfinalizedStruct,
    syntax: &Arc<Mutex<Syntax>>,
    include_refs: bool,
) -> Result<FinalizedStruct, ParsingError> {
    let mut finalized_fields = Vec::default();
    for field in structure.fields {
        let field = field.await?;
        let mut field_type = field.field.field_type.finalize(syntax.clone()).await;
        if include_refs {
            field_type = FinalizedTypes::Reference(Box::new(field_type));
        }
        finalized_fields.push(FinalizedMemberField {
            modifiers: field.modifiers,
            attributes: field.attributes,
            field: FinalizedField { field_type, name: field.field.name },
        })
    }

    let output = FinalizedStruct {
        generics: finalize_generics(syntax, structure.generics).await?,
        fields: finalized_fields,
        data: structure.data,
    };

    return Ok(output);
}
