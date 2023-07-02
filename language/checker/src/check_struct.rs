use std::sync::{Arc, Mutex};
use syntax::ParsingError;
use syntax::code::{FinalizedField, FinalizedMemberField};
use syntax::r#struct::{FinalizedStruct, UnfinalizedStruct};
use syntax::syntax::Syntax;
use crate::finalize_generics;
use crate::output::TypesChecker;

pub async fn verify_struct(_process_manager: &TypesChecker, structure: UnfinalizedStruct,
                           syntax: &Arc<Mutex<Syntax>>) -> Result<FinalizedStruct, ParsingError> {
    let mut finalized_fields = Vec::new();
    for field in structure.fields {
        let field = field.await?;
        finalized_fields.push(FinalizedMemberField { modifiers: field.modifiers, attributes: field.attributes,
            field: FinalizedField { field_type: field.field.field_type.finalize(syntax.clone()).await, name: field.field.name } })
    }
    return Ok(FinalizedStruct {
        generics: finalize_generics(syntax, structure.generics).await?,
        fields: finalized_fields,
        data: structure.data,
    });
}