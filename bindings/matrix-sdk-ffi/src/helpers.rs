use std::sync::Arc;

pub(crate) fn unwrap_or_clone_arc<T: Clone>(arc: Arc<T>) -> T {
    Arc::try_unwrap(arc).unwrap_or_else(|x| (*x).clone())
}

#[matrix_sdk_ffi_macros::export]
pub fn is_room_alias_valid(room_alias: String) -> bool {
    ruma_identifiers_validation::room_alias_id::validate(&room_alias).is_ok()
}