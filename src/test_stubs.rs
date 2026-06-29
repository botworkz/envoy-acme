// No-op stubs for every `envoy_dynamic_module_callback_*` symbol declared in the
// envoy-proxy-dynamic-modules-rust-sdk ABI. These satisfy the linker when cargo
// tarpaulin (or any coverage tool) forces all SDK code live at link time.
// At runtime in production, Envoy itself provides the real implementations.
#![allow(non_snake_case)]
#![allow(unused_variables)]
#![allow(clippy::missing_safety_doc)]

use envoy_proxy_dynamic_modules_rust_sdk::abi::*;

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_log(
    _level: envoy_dynamic_module_type_log_level,
    _message: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_log_enabled(
    _level: envoy_dynamic_module_type_log_level,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_get_concurrency() -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_is_validation_mode() -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_register_function(
    _key: envoy_dynamic_module_type_module_buffer,
    _function_ptr: *mut ::std::os::raw::c_void,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_get_function(
    _key: envoy_dynamic_module_type_module_buffer,
    _function_ptr_out: *mut *mut ::std::os::raw::c_void,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_register_shared_data(
    _key: envoy_dynamic_module_type_module_buffer,
    _data_ptr: *mut ::std::os::raw::c_void,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_get_shared_data(
    _key: envoy_dynamic_module_type_module_buffer,
    _data_ptr_out: *mut *mut ::std::os::raw::c_void,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_config_define_counter(
    _filter_config_envoy_ptr: envoy_dynamic_module_type_http_filter_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _label_names: *mut envoy_dynamic_module_type_module_buffer,
    _label_names_length: usize,
    _counter_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_increment_counter(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_config_define_gauge(
    _filter_config_envoy_ptr: envoy_dynamic_module_type_http_filter_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _label_names: *mut envoy_dynamic_module_type_module_buffer,
    _label_names_length: usize,
    _gauge_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_increment_gauge(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_decrement_gauge(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_set_gauge(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_config_define_histogram(
    _filter_config_envoy_ptr: envoy_dynamic_module_type_http_filter_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _label_names: *mut envoy_dynamic_module_type_module_buffer,
    _label_names_length: usize,
    _histogram_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_record_histogram_value(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_header(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _header_type: envoy_dynamic_module_type_http_header_type,
    _key: envoy_dynamic_module_type_module_buffer,
    _result_buffer: *mut envoy_dynamic_module_type_envoy_buffer,
    _index: usize,
    _optional_size: *mut usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_headers_size(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _header_type: envoy_dynamic_module_type_http_header_type,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_headers(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _header_type: envoy_dynamic_module_type_http_header_type,
    _result_headers: *mut envoy_dynamic_module_type_envoy_http_header,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_add_header(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _header_type: envoy_dynamic_module_type_http_header_type,
    _key: envoy_dynamic_module_type_module_buffer,
    _value: envoy_dynamic_module_type_module_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_set_header(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _header_type: envoy_dynamic_module_type_http_header_type,
    _key: envoy_dynamic_module_type_module_buffer,
    _value: envoy_dynamic_module_type_module_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_send_response(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _status_code: u32,
    _headers_vector: *mut envoy_dynamic_module_type_module_http_header,
    _headers_vector_size: usize,
    _body: envoy_dynamic_module_type_module_buffer,
    _details: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_send_response_headers(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _headers_vector: *mut envoy_dynamic_module_type_module_http_header,
    _headers_vector_size: usize,
    _end_stream: bool,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_send_response_data(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _data: envoy_dynamic_module_type_module_buffer,
    _end_stream: bool,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_send_response_trailers(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _trailers_vector: *mut envoy_dynamic_module_type_module_http_header,
    _trailers_vector_size: usize,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_body_size(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _body_type: envoy_dynamic_module_type_http_body_type,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_body_chunks(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _body_type: envoy_dynamic_module_type_http_body_type,
    _result_buffer_vector: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_body_chunks_size(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _body_type: envoy_dynamic_module_type_http_body_type,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_append_body(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _body_type: envoy_dynamic_module_type_http_body_type,
    _data: envoy_dynamic_module_type_module_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_drain_body(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _body_type: envoy_dynamic_module_type_http_body_type,
    _number_of_bytes: usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_received_buffered_request_body(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_received_buffered_response_body(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_set_dynamic_metadata_number(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _ns: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _value: f64,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_metadata_number(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _metadata_source: envoy_dynamic_module_type_metadata_source,
    _ns: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _result: *mut f64,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_set_dynamic_metadata_string(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _ns: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _value: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_metadata_string(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _metadata_source: envoy_dynamic_module_type_metadata_source,
    _ns: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_set_dynamic_metadata_bool(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _ns: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _value: bool,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_metadata_bool(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _metadata_source: envoy_dynamic_module_type_metadata_source,
    _ns: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _result: *mut bool,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_metadata_keys_count(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _metadata_source: envoy_dynamic_module_type_metadata_source,
    _ns: envoy_dynamic_module_type_module_buffer,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_metadata_keys(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _metadata_source: envoy_dynamic_module_type_metadata_source,
    _ns: envoy_dynamic_module_type_module_buffer,
    _result_buffer_vector: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_metadata_namespaces_count(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _metadata_source: envoy_dynamic_module_type_metadata_source,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_metadata_namespaces(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _metadata_source: envoy_dynamic_module_type_metadata_source,
    _result_buffer_vector: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_add_dynamic_metadata_list_number(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _ns: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _value: f64,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_add_dynamic_metadata_list_string(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _ns: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _value: envoy_dynamic_module_type_module_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_add_dynamic_metadata_list_bool(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _ns: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _value: bool,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_metadata_list_size(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _metadata_source: envoy_dynamic_module_type_metadata_source,
    _ns: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _result: *mut usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_metadata_list_number(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _metadata_source: envoy_dynamic_module_type_metadata_source,
    _ns: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _index: usize,
    _result: *mut f64,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_metadata_list_string(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _metadata_source: envoy_dynamic_module_type_metadata_source,
    _ns: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _index: usize,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_metadata_list_bool(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _metadata_source: envoy_dynamic_module_type_metadata_source,
    _ns: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _index: usize,
    _result: *mut bool,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_set_filter_state_bytes(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _key: envoy_dynamic_module_type_module_buffer,
    _value: envoy_dynamic_module_type_module_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_filter_state_bytes(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _key: envoy_dynamic_module_type_module_buffer,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_set_filter_state_typed(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _key: envoy_dynamic_module_type_module_buffer,
    _value: envoy_dynamic_module_type_module_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_filter_state_typed(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _key: envoy_dynamic_module_type_module_buffer,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_add_custom_flag(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _flag: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_scheduler_new(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
) -> envoy_dynamic_module_type_http_filter_scheduler_module_ptr {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_scheduler_commit(
    _scheduler_module_ptr: envoy_dynamic_module_type_http_filter_scheduler_module_ptr,
    _event_id: u64,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_scheduler_delete(
    _scheduler_module_ptr: envoy_dynamic_module_type_http_filter_scheduler_module_ptr,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_config_scheduler_new(
    _filter_config_envoy_ptr: envoy_dynamic_module_type_http_filter_config_envoy_ptr,
) -> envoy_dynamic_module_type_http_filter_config_scheduler_module_ptr {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_config_scheduler_delete(
    _scheduler_module_ptr: envoy_dynamic_module_type_http_filter_config_scheduler_module_ptr,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_config_scheduler_commit(
    _scheduler_module_ptr: envoy_dynamic_module_type_http_filter_config_scheduler_module_ptr,
    _event_id: u64,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_clear_route_cache(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_get_attribute_string(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _attribute_id: envoy_dynamic_module_type_attribute_id,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_get_attribute_int(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _attribute_id: envoy_dynamic_module_type_attribute_id,
    _result: *mut u64,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_get_attribute_bool(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _attribute_id: envoy_dynamic_module_type_attribute_id,
    _result: *mut bool,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_http_callout(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _callout_id_out: *mut u64,
    _cluster_name: envoy_dynamic_module_type_module_buffer,
    _headers: *mut envoy_dynamic_module_type_module_http_header,
    _headers_size: usize,
    _body: envoy_dynamic_module_type_module_buffer,
    _timeout_milliseconds: u64,
) -> envoy_dynamic_module_type_http_callout_init_result {
    envoy_dynamic_module_type_http_callout_init_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_start_http_stream(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _stream_id_out: *mut u64,
    _cluster_name: envoy_dynamic_module_type_module_buffer,
    _headers: *mut envoy_dynamic_module_type_module_http_header,
    _headers_size: usize,
    _body: envoy_dynamic_module_type_module_buffer,
    _end_stream: bool,
    _timeout_milliseconds: u64,
) -> envoy_dynamic_module_type_http_callout_init_result {
    envoy_dynamic_module_type_http_callout_init_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_reset_http_stream(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _stream_id: u64,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_stream_send_data(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _stream_id: u64,
    _data: envoy_dynamic_module_type_module_buffer,
    _end_stream: bool,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_stream_send_trailers(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _stream_id: u64,
    _trailers: *mut envoy_dynamic_module_type_module_http_header,
    _trailers_size: usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_config_http_callout(
    _filter_config_envoy_ptr: envoy_dynamic_module_type_http_filter_config_envoy_ptr,
    _callout_id_out: *mut u64,
    _cluster_name: envoy_dynamic_module_type_module_buffer,
    _headers: *mut envoy_dynamic_module_type_module_http_header,
    _headers_size: usize,
    _body: envoy_dynamic_module_type_module_buffer,
    _timeout_milliseconds: u64,
) -> envoy_dynamic_module_type_http_callout_init_result {
    envoy_dynamic_module_type_http_callout_init_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_config_start_http_stream(
    _filter_config_envoy_ptr: envoy_dynamic_module_type_http_filter_config_envoy_ptr,
    _stream_id_out: *mut u64,
    _cluster_name: envoy_dynamic_module_type_module_buffer,
    _headers: *mut envoy_dynamic_module_type_module_http_header,
    _headers_size: usize,
    _body: envoy_dynamic_module_type_module_buffer,
    _end_stream: bool,
    _timeout_milliseconds: u64,
) -> envoy_dynamic_module_type_http_callout_init_result {
    envoy_dynamic_module_type_http_callout_init_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_config_reset_http_stream(
    _filter_config_envoy_ptr: envoy_dynamic_module_type_http_filter_config_envoy_ptr,
    _stream_id: u64,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_config_stream_send_data(
    _filter_config_envoy_ptr: envoy_dynamic_module_type_http_filter_config_envoy_ptr,
    _stream_id: u64,
    _data: envoy_dynamic_module_type_module_buffer,
    _end_stream: bool,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_config_stream_send_trailers(
    _filter_config_envoy_ptr: envoy_dynamic_module_type_http_filter_config_envoy_ptr,
    _stream_id: u64,
    _trailers: *mut envoy_dynamic_module_type_module_http_header,
    _trailers_size: usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_continue_decoding(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_continue_encoding(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_get_most_specific_route_config(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
) -> envoy_dynamic_module_type_http_filter_per_route_config_module_ptr {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_get_worker_index(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_set_socket_option_int(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _level: i64,
    _name: i64,
    _state: envoy_dynamic_module_type_socket_option_state,
    _direction: envoy_dynamic_module_type_socket_direction,
    _value: i64,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_set_socket_option_bytes(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _level: i64,
    _name: i64,
    _state: envoy_dynamic_module_type_socket_option_state,
    _direction: envoy_dynamic_module_type_socket_direction,
    _value: envoy_dynamic_module_type_module_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_socket_option_int(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _level: i64,
    _name: i64,
    _state: envoy_dynamic_module_type_socket_option_state,
    _direction: envoy_dynamic_module_type_socket_direction,
    _value_out: *mut i64,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_socket_option_bytes(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _level: i64,
    _name: i64,
    _state: envoy_dynamic_module_type_socket_option_state,
    _direction: envoy_dynamic_module_type_socket_direction,
    _value_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_buffer_limit(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
) -> u64 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_set_buffer_limit(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _limit: u64,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_active_span(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
) -> envoy_dynamic_module_type_span_envoy_ptr {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_span_set_tag(
    _span: envoy_dynamic_module_type_span_envoy_ptr,
    _key: envoy_dynamic_module_type_module_buffer,
    _value: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_span_set_operation(
    _span: envoy_dynamic_module_type_span_envoy_ptr,
    _operation: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_span_log(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _span: envoy_dynamic_module_type_span_envoy_ptr,
    _event: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_span_set_sampled(
    _span: envoy_dynamic_module_type_span_envoy_ptr,
    _sampled: bool,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_span_get_baggage(
    _span: envoy_dynamic_module_type_span_envoy_ptr,
    _key: envoy_dynamic_module_type_module_buffer,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_span_set_baggage(
    _span: envoy_dynamic_module_type_span_envoy_ptr,
    _key: envoy_dynamic_module_type_module_buffer,
    _value: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_span_get_trace_id(
    _span: envoy_dynamic_module_type_span_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_span_get_span_id(
    _span: envoy_dynamic_module_type_span_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_span_spawn_child(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _span: envoy_dynamic_module_type_span_envoy_ptr,
    _operation_name: envoy_dynamic_module_type_module_buffer,
) -> envoy_dynamic_module_type_child_span_module_ptr {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_child_span_finish(
    _span: envoy_dynamic_module_type_child_span_module_ptr,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_cluster_name(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_get_cluster_host_count(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _priority: u32,
    _total_count: *mut usize,
    _healthy_count: *mut usize,
    _degraded_count: *mut usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_set_upstream_override_host(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _host: envoy_dynamic_module_type_module_buffer,
    _strict: bool,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_reset_stream(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _reason: envoy_dynamic_module_type_http_filter_stream_reset_reason,
    _details: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_send_go_away_and_close(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _graceful: bool,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_filter_recreate_stream(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
    _headers: *mut envoy_dynamic_module_type_module_http_header,
    _headers_size: usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_http_clear_route_cluster_cache(
    _filter_envoy_ptr: envoy_dynamic_module_type_http_filter_envoy_ptr,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_set_socket_option_int(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _level: i64,
    _name: i64,
    _state: envoy_dynamic_module_type_socket_option_state,
    _value: i64,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_set_socket_option_bytes(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _level: i64,
    _name: i64,
    _state: envoy_dynamic_module_type_socket_option_state,
    _value: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_get_socket_option_int(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _level: i64,
    _name: i64,
    _state: envoy_dynamic_module_type_socket_option_state,
    _value_out: *mut i64,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_get_socket_option_bytes(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _level: i64,
    _name: i64,
    _state: envoy_dynamic_module_type_socket_option_state,
    _value_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_get_socket_options_size(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_get_socket_options(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _options_out: *mut envoy_dynamic_module_type_socket_option,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_read_buffer_chunks_size(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_read_buffer_size(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_read_buffer_chunks(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _result_buffer_vector: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_write_buffer_chunks_size(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_write_buffer_size(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_write_buffer_chunks(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _result_buffer_vector: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_drain_read_buffer(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _length: usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_drain_write_buffer(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _length: usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_prepend_read_buffer(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _data: envoy_dynamic_module_type_module_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_append_read_buffer(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _data: envoy_dynamic_module_type_module_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_prepend_write_buffer(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _data: envoy_dynamic_module_type_module_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_append_write_buffer(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _data: envoy_dynamic_module_type_module_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_write(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _data: envoy_dynamic_module_type_module_buffer,
    _end_stream: bool,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_inject_read_data(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _data: envoy_dynamic_module_type_module_buffer,
    _end_stream: bool,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_inject_write_data(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _data: envoy_dynamic_module_type_module_buffer,
    _end_stream: bool,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_continue_reading(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_close(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _close_type: envoy_dynamic_module_type_network_connection_close_type,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_connection_id(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
) -> u64 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_remote_address(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _address_out: *mut envoy_dynamic_module_type_envoy_buffer,
    _port_out: *mut u32,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_local_address(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _address_out: *mut envoy_dynamic_module_type_envoy_buffer,
    _port_out: *mut u32,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_is_ssl(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_disable_close(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _disabled: bool,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_close_with_details(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _close_type: envoy_dynamic_module_type_network_connection_close_type,
    _details: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_requested_server_name(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _result_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_direct_remote_address(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _address_out: *mut envoy_dynamic_module_type_envoy_buffer,
    _port_out: *mut u32,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_ssl_uri_sans_size(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_ssl_uri_sans(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _sans_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_ssl_dns_sans_size(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_ssl_dns_sans(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _sans_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_ssl_subject(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _result_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_set_filter_state_bytes(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _key: envoy_dynamic_module_type_module_buffer,
    _value: envoy_dynamic_module_type_module_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_get_filter_state_bytes(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _key: envoy_dynamic_module_type_module_buffer,
    _value_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_set_filter_state_typed(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _key: envoy_dynamic_module_type_module_buffer,
    _value: envoy_dynamic_module_type_module_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_get_filter_state_typed(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _key: envoy_dynamic_module_type_module_buffer,
    _value_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_set_dynamic_metadata_string(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _filter_namespace: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _value: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_get_dynamic_metadata_string(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _filter_namespace: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _value_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_set_dynamic_metadata_number(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _filter_namespace: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _value: f64,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_get_dynamic_metadata_number(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _filter_namespace: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _result: *mut f64,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_set_dynamic_metadata_bool(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _filter_namespace: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _value: bool,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_get_dynamic_metadata_bool(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _filter_namespace: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _result: *mut bool,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_http_callout(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _callout_id_out: *mut u64,
    _cluster_name: envoy_dynamic_module_type_module_buffer,
    _headers: *mut envoy_dynamic_module_type_module_http_header,
    _headers_size: usize,
    _body: envoy_dynamic_module_type_module_buffer,
    _timeout_milliseconds: u64,
) -> envoy_dynamic_module_type_http_callout_init_result {
    envoy_dynamic_module_type_http_callout_init_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_config_define_counter(
    _config_envoy_ptr: envoy_dynamic_module_type_network_filter_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _counter_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_increment_counter(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _id: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_config_define_gauge(
    _config_envoy_ptr: envoy_dynamic_module_type_network_filter_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _gauge_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_set_gauge(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _id: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_increment_gauge(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _id: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_decrement_gauge(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _id: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_config_define_histogram(
    _config_envoy_ptr: envoy_dynamic_module_type_network_filter_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _histogram_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_record_histogram_value(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _id: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_cluster_host_count(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _cluster_name: envoy_dynamic_module_type_module_buffer,
    _priority: u32,
    _total_count: *mut usize,
    _healthy_count: *mut usize,
    _degraded_count: *mut usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_upstream_host_address(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _address_out: *mut envoy_dynamic_module_type_envoy_buffer,
    _port_out: *mut u32,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_upstream_host_hostname(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _hostname_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_upstream_host_cluster(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _cluster_name_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_has_upstream_host(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_start_upstream_secure_transport(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_connection_state(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
) -> envoy_dynamic_module_type_network_connection_state {
    envoy_dynamic_module_type_network_connection_state::Open
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_read_disable(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _disable: bool,
) -> envoy_dynamic_module_type_network_read_disable_status {
    envoy_dynamic_module_type_network_read_disable_status::NoTransition
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_read_enabled(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_is_half_close_enabled(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_enable_half_close(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _enabled: bool,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_buffer_limit(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_set_buffer_limits(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
    _limit: u32,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_above_high_watermark(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_scheduler_new(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
) -> envoy_dynamic_module_type_network_filter_scheduler_module_ptr {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_scheduler_commit(
    _scheduler_module_ptr: envoy_dynamic_module_type_network_filter_scheduler_module_ptr,
    _event_id: u64,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_scheduler_delete(
    _scheduler_module_ptr: envoy_dynamic_module_type_network_filter_scheduler_module_ptr,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_config_scheduler_new(
    _filter_config_envoy_ptr: envoy_dynamic_module_type_network_filter_config_envoy_ptr,
) -> envoy_dynamic_module_type_network_filter_config_scheduler_module_ptr {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_config_scheduler_delete(
    _scheduler_module_ptr: envoy_dynamic_module_type_network_filter_config_scheduler_module_ptr,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_config_scheduler_commit(
    _scheduler_module_ptr: envoy_dynamic_module_type_network_filter_config_scheduler_module_ptr,
    _event_id: u64,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_network_filter_get_worker_index(
    _filter_envoy_ptr: envoy_dynamic_module_type_network_filter_envoy_ptr,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_buffer_chunk(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _chunk_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_drain_buffer(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _length: usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_set_detected_transport_protocol(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _protocol: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_set_requested_server_name(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_set_requested_application_protocols(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _protocols: *mut envoy_dynamic_module_type_module_buffer,
    _protocols_count: usize,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_set_ja3_hash(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _hash: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_set_ja4_hash(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _hash: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_requested_server_name(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _result_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_detected_transport_protocol(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _result_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_requested_application_protocols_size(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_requested_application_protocols(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _protocols_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_ja3_hash(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _result_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_ja4_hash(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _result_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_is_ssl(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_ssl_uri_sans_size(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_ssl_uri_sans(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _sans_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_ssl_dns_sans_size(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_ssl_dns_sans(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _sans_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_ssl_subject(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _result_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_remote_address(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _address_out: *mut envoy_dynamic_module_type_envoy_buffer,
    _port_out: *mut u32,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_direct_remote_address(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _address_out: *mut envoy_dynamic_module_type_envoy_buffer,
    _port_out: *mut u32,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_local_address(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _address_out: *mut envoy_dynamic_module_type_envoy_buffer,
    _port_out: *mut u32,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_direct_local_address(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _address_out: *mut envoy_dynamic_module_type_envoy_buffer,
    _port_out: *mut u32,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_original_dst(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _address_out: *mut envoy_dynamic_module_type_envoy_buffer,
    _port_out: *mut u32,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_address_type(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
) -> envoy_dynamic_module_type_address_type {
    envoy_dynamic_module_type_address_type::Unknown
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_is_local_address_restored(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_set_remote_address(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _address: envoy_dynamic_module_type_module_buffer,
    _port: u32,
    _is_ipv6: bool,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_restore_local_address(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _address: envoy_dynamic_module_type_module_buffer,
    _port: u32,
    _is_ipv6: bool,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_continue_filter_chain(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _success: bool,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_use_original_dst(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _use_original_dst: bool,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_close_socket(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _details: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_write_to_socket(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _data: envoy_dynamic_module_type_module_buffer,
) -> i64 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_socket_fd(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
) -> i64 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_set_socket_option_int(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _level: i64,
    _name: i64,
    _value: i64,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_set_socket_option_bytes(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _level: i64,
    _name: i64,
    _value: envoy_dynamic_module_type_module_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_socket_option_int(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _level: i64,
    _name: i64,
    _value_out: *mut i64,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_socket_option_bytes(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _level: i64,
    _name: i64,
    _value_out: *mut ::std::os::raw::c_char,
    _value_size: usize,
    _actual_size_out: *mut usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_set_filter_state(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _key: envoy_dynamic_module_type_module_buffer,
    _value: envoy_dynamic_module_type_module_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_filter_state(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _key: envoy_dynamic_module_type_module_buffer,
    _value_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_set_downstream_transport_failure_reason(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _reason: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_connection_start_time_ms(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
) -> u64 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_dynamic_metadata_string(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _filter_namespace: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _value_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_set_dynamic_metadata_string(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _filter_namespace: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _value: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_dynamic_metadata_number(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _filter_namespace: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _result: *mut f64,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_set_dynamic_metadata_number(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _filter_namespace: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _value: f64,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_max_read_bytes(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_config_define_counter(
    _config_envoy_ptr: envoy_dynamic_module_type_listener_filter_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _counter_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_increment_counter(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _id: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_config_define_gauge(
    _config_envoy_ptr: envoy_dynamic_module_type_listener_filter_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _gauge_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_set_gauge(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _id: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_increment_gauge(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _id: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_decrement_gauge(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _id: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_config_define_histogram(
    _config_envoy_ptr: envoy_dynamic_module_type_listener_filter_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _histogram_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_record_histogram_value(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _id: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_http_callout(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
    _callout_id_out: *mut u64,
    _cluster_name: envoy_dynamic_module_type_module_buffer,
    _headers: *mut envoy_dynamic_module_type_module_http_header,
    _headers_size: usize,
    _body: envoy_dynamic_module_type_module_buffer,
    _timeout_milliseconds: u64,
) -> envoy_dynamic_module_type_http_callout_init_result {
    envoy_dynamic_module_type_http_callout_init_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_scheduler_new(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
) -> envoy_dynamic_module_type_listener_filter_scheduler_module_ptr {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_scheduler_commit(
    _scheduler_module_ptr: envoy_dynamic_module_type_listener_filter_scheduler_module_ptr,
    _event_id: u64,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_scheduler_delete(
    _scheduler_module_ptr: envoy_dynamic_module_type_listener_filter_scheduler_module_ptr,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_config_scheduler_new(
    _filter_config_envoy_ptr: envoy_dynamic_module_type_listener_filter_config_envoy_ptr,
) -> envoy_dynamic_module_type_listener_filter_config_scheduler_module_ptr {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_config_scheduler_delete(
    _scheduler_module_ptr: envoy_dynamic_module_type_listener_filter_config_scheduler_module_ptr,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_config_scheduler_commit(
    _scheduler_module_ptr: envoy_dynamic_module_type_listener_filter_config_scheduler_module_ptr,
    _event_id: u64,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_listener_filter_get_worker_index(
    _filter_envoy_ptr: envoy_dynamic_module_type_listener_filter_envoy_ptr,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_udp_listener_filter_get_datagram_data_chunks_size(
    _filter_envoy_ptr: envoy_dynamic_module_type_udp_listener_filter_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_udp_listener_filter_get_datagram_data_chunks(
    _filter_envoy_ptr: envoy_dynamic_module_type_udp_listener_filter_envoy_ptr,
    _chunks_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_udp_listener_filter_get_datagram_data_size(
    _filter_envoy_ptr: envoy_dynamic_module_type_udp_listener_filter_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_udp_listener_filter_set_datagram_data(
    _filter_envoy_ptr: envoy_dynamic_module_type_udp_listener_filter_envoy_ptr,
    _data: envoy_dynamic_module_type_module_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_udp_listener_filter_get_peer_address(
    _filter_envoy_ptr: envoy_dynamic_module_type_udp_listener_filter_envoy_ptr,
    _address_out: *mut envoy_dynamic_module_type_envoy_buffer,
    _port_out: *mut u32,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_udp_listener_filter_get_local_address(
    _filter_envoy_ptr: envoy_dynamic_module_type_udp_listener_filter_envoy_ptr,
    _address_out: *mut envoy_dynamic_module_type_envoy_buffer,
    _port_out: *mut u32,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_udp_listener_filter_send_datagram(
    _filter_envoy_ptr: envoy_dynamic_module_type_udp_listener_filter_envoy_ptr,
    _data: envoy_dynamic_module_type_module_buffer,
    _peer_address: envoy_dynamic_module_type_module_buffer,
    _peer_port: u32,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_udp_listener_filter_config_define_counter(
    _config_envoy_ptr: envoy_dynamic_module_type_udp_listener_filter_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _counter_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_udp_listener_filter_increment_counter(
    _filter_envoy_ptr: envoy_dynamic_module_type_udp_listener_filter_envoy_ptr,
    _id: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_udp_listener_filter_config_define_gauge(
    _config_envoy_ptr: envoy_dynamic_module_type_udp_listener_filter_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _gauge_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_udp_listener_filter_set_gauge(
    _filter_envoy_ptr: envoy_dynamic_module_type_udp_listener_filter_envoy_ptr,
    _id: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_udp_listener_filter_increment_gauge(
    _filter_envoy_ptr: envoy_dynamic_module_type_udp_listener_filter_envoy_ptr,
    _id: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_udp_listener_filter_decrement_gauge(
    _filter_envoy_ptr: envoy_dynamic_module_type_udp_listener_filter_envoy_ptr,
    _id: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_udp_listener_filter_config_define_histogram(
    _config_envoy_ptr: envoy_dynamic_module_type_udp_listener_filter_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _histogram_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_udp_listener_filter_record_histogram_value(
    _filter_envoy_ptr: envoy_dynamic_module_type_udp_listener_filter_envoy_ptr,
    _id: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_udp_listener_filter_get_worker_index(
    _filter_envoy_ptr: envoy_dynamic_module_type_udp_listener_filter_envoy_ptr,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_headers_size(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _header_type: envoy_dynamic_module_type_http_header_type,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_headers(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _header_type: envoy_dynamic_module_type_http_header_type,
    _result_headers: *mut envoy_dynamic_module_type_envoy_http_header,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_header_value(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _header_type: envoy_dynamic_module_type_http_header_type,
    _key: envoy_dynamic_module_type_module_buffer,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
    _index: usize,
    _total_count_out: *mut usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_response_code(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_response_code_details(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_has_response_flag(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _flag: envoy_dynamic_module_type_response_flag,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_response_flags(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> u64 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_protocol(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_timing_info(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _timing_out: *mut envoy_dynamic_module_type_timing_info,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_bytes_info(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _bytes_out: *mut envoy_dynamic_module_type_bytes_info,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_is_health_check(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_route_name(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_virtual_cluster_name(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_attempt_count(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_connection_termination_details(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_remote_address(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _address_out: *mut envoy_dynamic_module_type_envoy_buffer,
    _port_out: *mut u32,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_local_address(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _address_out: *mut envoy_dynamic_module_type_envoy_buffer,
    _port_out: *mut u32,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_direct_remote_address(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _address_out: *mut envoy_dynamic_module_type_envoy_buffer,
    _port_out: *mut u32,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_direct_local_address(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _address_out: *mut envoy_dynamic_module_type_envoy_buffer,
    _port_out: *mut u32,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_remote_address(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _address_out: *mut envoy_dynamic_module_type_envoy_buffer,
    _port_out: *mut u32,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_local_address(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _address_out: *mut envoy_dynamic_module_type_envoy_buffer,
    _port_out: *mut u32,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_cluster(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_host(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_transport_failure_reason(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_connection_id(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> u64 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_tls_cipher(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_tls_session_id(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_tls_version(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_peer_subject(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_peer_issuer(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_local_subject(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_peer_cert_digest(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_peer_cert_v_start(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> i64 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_peer_cert_v_end(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> i64 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_peer_uri_san_size(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_peer_uri_san(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _sans_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_local_uri_san_size(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_local_uri_san(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _sans_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_peer_dns_san_size(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_peer_dns_san(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _sans_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_local_dns_san_size(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_local_dns_san(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _sans_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_connection_id(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> u64 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_is_mtls(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_requested_server_name(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_tls_version(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_peer_subject(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_peer_cert_digest(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_tls_cipher(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_tls_session_id(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_peer_issuer(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_peer_serial(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_peer_fingerprint_1(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_local_subject(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_peer_cert_presented(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_peer_cert_validated(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_peer_cert_v_start(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> i64 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_peer_cert_v_end(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> i64 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_peer_uri_san_size(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_peer_uri_san(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _sans_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_local_uri_san_size(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_local_uri_san(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _sans_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_peer_dns_san_size(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_peer_dns_san(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _sans_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_local_dns_san_size(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_local_dns_san(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _sans_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_dynamic_metadata(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _filter_name: envoy_dynamic_module_type_module_buffer,
    _path: envoy_dynamic_module_type_module_buffer,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_filter_state(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _key: envoy_dynamic_module_type_module_buffer,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_request_id(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_local_reply_body(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_trace_id(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_span_id(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_is_trace_sampled(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_ja3_hash(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_ja4_hash(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_downstream_transport_failure_reason(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_request_headers_bytes(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> u64 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_response_headers_bytes(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> u64 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_response_trailers_bytes(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> u64 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_protocol(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_upstream_pool_ready_duration_ns(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> i64 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_attribute_string(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _attribute_id: envoy_dynamic_module_type_attribute_id,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_attribute_int(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _attribute_id: envoy_dynamic_module_type_attribute_id,
    _result: *mut u64,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_attribute_bool(
    _logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
    _attribute_id: envoy_dynamic_module_type_attribute_id,
    _result: *mut bool,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_config_define_counter(
    _config_envoy_ptr: envoy_dynamic_module_type_access_logger_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _counter_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_increment_counter(
    _config_envoy_ptr: envoy_dynamic_module_type_access_logger_config_envoy_ptr,
    _id: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_config_define_gauge(
    _config_envoy_ptr: envoy_dynamic_module_type_access_logger_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _gauge_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_set_gauge(
    _config_envoy_ptr: envoy_dynamic_module_type_access_logger_config_envoy_ptr,
    _id: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_increment_gauge(
    _config_envoy_ptr: envoy_dynamic_module_type_access_logger_config_envoy_ptr,
    _id: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_decrement_gauge(
    _config_envoy_ptr: envoy_dynamic_module_type_access_logger_config_envoy_ptr,
    _id: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_config_define_histogram(
    _config_envoy_ptr: envoy_dynamic_module_type_access_logger_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _histogram_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_record_histogram_value(
    _config_envoy_ptr: envoy_dynamic_module_type_access_logger_config_envoy_ptr,
    _id: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_access_logger_get_worker_index(
    _access_logger_envoy_ptr: envoy_dynamic_module_type_access_logger_envoy_ptr,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_config_scheduler_new(
    _extension_config_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_config_envoy_ptr,
) -> envoy_dynamic_module_type_bootstrap_extension_config_scheduler_module_ptr {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_config_scheduler_delete(
    _scheduler_module_ptr: envoy_dynamic_module_type_bootstrap_extension_config_scheduler_module_ptr,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_config_scheduler_commit(
    _scheduler_module_ptr: envoy_dynamic_module_type_bootstrap_extension_config_scheduler_module_ptr,
    _event_id: u64,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_http_callout(
    _extension_config_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_config_envoy_ptr,
    _callout_id_out: *mut u64,
    _cluster_name: envoy_dynamic_module_type_module_buffer,
    _headers: *mut envoy_dynamic_module_type_module_http_header,
    _headers_size: usize,
    _body: envoy_dynamic_module_type_module_buffer,
    _timeout_milliseconds: u64,
) -> envoy_dynamic_module_type_http_callout_init_result {
    envoy_dynamic_module_type_http_callout_init_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_config_signal_init_complete(
    _extension_config_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_config_envoy_ptr,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_get_counter_value(
    _extension_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _value_ptr: *mut u64,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_get_gauge_value(
    _extension_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _value_ptr: *mut u64,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_get_histogram_summary(
    _extension_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _sample_count_ptr: *mut u64,
    _sample_sum_ptr: *mut f64,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_iterate_counters(
    _extension_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_envoy_ptr,
    _iterator_fn: envoy_dynamic_module_type_counter_iterator_fn,
    _user_data: *mut ::std::os::raw::c_void,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_iterate_gauges(
    _extension_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_envoy_ptr,
    _iterator_fn: envoy_dynamic_module_type_gauge_iterator_fn,
    _user_data: *mut ::std::os::raw::c_void,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_config_define_counter(
    _config_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _label_names: *mut envoy_dynamic_module_type_module_buffer,
    _label_names_length: usize,
    _counter_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_config_increment_counter(
    _config_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_config_define_gauge(
    _config_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _label_names: *mut envoy_dynamic_module_type_module_buffer,
    _label_names_length: usize,
    _gauge_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_config_set_gauge(
    _config_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_config_increment_gauge(
    _config_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_config_decrement_gauge(
    _config_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_config_define_histogram(
    _config_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _label_names: *mut envoy_dynamic_module_type_module_buffer,
    _label_names_length: usize,
    _histogram_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_config_record_histogram_value(
    _config_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_timer_new(
    _extension_config_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_config_envoy_ptr,
) -> envoy_dynamic_module_type_bootstrap_extension_timer_module_ptr {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_timer_enable(
    _timer_ptr: envoy_dynamic_module_type_bootstrap_extension_timer_module_ptr,
    _delay_milliseconds: u64,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_timer_disable(
    _timer_ptr: envoy_dynamic_module_type_bootstrap_extension_timer_module_ptr,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_timer_enabled(
    _timer_ptr: envoy_dynamic_module_type_bootstrap_extension_timer_module_ptr,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_timer_delete(
    _timer_ptr: envoy_dynamic_module_type_bootstrap_extension_timer_module_ptr,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_file_watcher_add_watch(
    _extension_config_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_config_envoy_ptr,
    _path: envoy_dynamic_module_type_module_buffer,
    _events: u32,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_admin_set_response(
    _extension_config_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_config_envoy_ptr,
    _response_body: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_register_admin_handler(
    _extension_config_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_config_envoy_ptr,
    _path_prefix: envoy_dynamic_module_type_module_buffer,
    _help_text: envoy_dynamic_module_type_module_buffer,
    _removable: bool,
    _mutates_server_state: bool,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_remove_admin_handler(
    _extension_config_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_config_envoy_ptr,
    _path_prefix: envoy_dynamic_module_type_module_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_enable_cluster_lifecycle(
    _extension_config_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_config_envoy_ptr,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_bootstrap_extension_enable_listener_lifecycle(
    _extension_config_envoy_ptr: envoy_dynamic_module_type_bootstrap_extension_config_envoy_ptr,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_add_hosts(
    _cluster_envoy_ptr: envoy_dynamic_module_type_cluster_envoy_ptr,
    _priority: u32,
    _addresses: *const envoy_dynamic_module_type_module_buffer,
    _weights: *const u32,
    _regions: *const envoy_dynamic_module_type_module_buffer,
    _zones: *const envoy_dynamic_module_type_module_buffer,
    _sub_zones: *const envoy_dynamic_module_type_module_buffer,
    _metadata_pairs: *const envoy_dynamic_module_type_module_buffer,
    _metadata_pairs_per_host: usize,
    _count: usize,
    _result_host_ptrs: *mut envoy_dynamic_module_type_cluster_host_envoy_ptr,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_remove_hosts(
    _cluster_envoy_ptr: envoy_dynamic_module_type_cluster_envoy_ptr,
    _host_envoy_ptrs: *const envoy_dynamic_module_type_cluster_host_envoy_ptr,
    _count: usize,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_update_host_health(
    _cluster_envoy_ptr: envoy_dynamic_module_type_cluster_envoy_ptr,
    _host_envoy_ptr: envoy_dynamic_module_type_cluster_host_envoy_ptr,
    _health_status: envoy_dynamic_module_type_host_health,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_find_host_by_address(
    _cluster_envoy_ptr: envoy_dynamic_module_type_cluster_envoy_ptr,
    _address: envoy_dynamic_module_type_module_buffer,
) -> envoy_dynamic_module_type_cluster_host_envoy_ptr {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_pre_init_complete(
    _cluster_envoy_ptr: envoy_dynamic_module_type_cluster_envoy_ptr,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_healthy_host_count(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _priority: u32,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_healthy_host(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
) -> envoy_dynamic_module_type_cluster_host_envoy_ptr {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_cluster_name(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_hosts_count(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _priority: u32,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_degraded_hosts_count(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _priority: u32,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_priority_set_size(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_healthy_host_address(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_healthy_host_weight(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_host_health(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
) -> envoy_dynamic_module_type_host_health {
    envoy_dynamic_module_type_host_health::Unhealthy
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_host_health_by_address(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _address: envoy_dynamic_module_type_module_buffer,
    _result: *mut envoy_dynamic_module_type_host_health,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_find_host_by_address(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _address: envoy_dynamic_module_type_module_buffer,
) -> envoy_dynamic_module_type_cluster_host_envoy_ptr {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_host(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
) -> envoy_dynamic_module_type_cluster_host_envoy_ptr {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_host_address(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_host_weight(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_host_stat(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
    _stat: envoy_dynamic_module_type_host_stat,
) -> u64 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_host_locality(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
    _region: *mut envoy_dynamic_module_type_envoy_buffer,
    _zone: *mut envoy_dynamic_module_type_envoy_buffer,
    _sub_zone: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_set_host_data(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
    _data: usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_host_data(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
    _data: *mut usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_host_metadata_string(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
    _filter_name: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_host_metadata_number(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
    _filter_name: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _result: *mut f64,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_host_metadata_bool(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
    _filter_name: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _result: *mut bool,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_locality_count(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _priority: u32,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_locality_host_count(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _priority: u32,
    _locality_index: usize,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_locality_host_address(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _priority: u32,
    _locality_index: usize,
    _host_index: usize,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_locality_weight(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _priority: u32,
    _locality_index: usize,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_scheduler_new(
    _cluster_envoy_ptr: envoy_dynamic_module_type_cluster_envoy_ptr,
) -> envoy_dynamic_module_type_cluster_scheduler_module_ptr {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_scheduler_delete(
    _scheduler_module_ptr: envoy_dynamic_module_type_cluster_scheduler_module_ptr,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_scheduler_commit(
    _scheduler_module_ptr: envoy_dynamic_module_type_cluster_scheduler_module_ptr,
    _event_id: u64,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_config_define_counter(
    _cluster_config_envoy_ptr: envoy_dynamic_module_type_cluster_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _label_names: *mut envoy_dynamic_module_type_module_buffer,
    _label_names_length: usize,
    _counter_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_config_increment_counter(
    _cluster_config_envoy_ptr: envoy_dynamic_module_type_cluster_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_config_define_gauge(
    _cluster_config_envoy_ptr: envoy_dynamic_module_type_cluster_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _label_names: *mut envoy_dynamic_module_type_module_buffer,
    _label_names_length: usize,
    _gauge_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_config_set_gauge(
    _cluster_config_envoy_ptr: envoy_dynamic_module_type_cluster_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_config_increment_gauge(
    _cluster_config_envoy_ptr: envoy_dynamic_module_type_cluster_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_config_decrement_gauge(
    _cluster_config_envoy_ptr: envoy_dynamic_module_type_cluster_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_config_define_histogram(
    _cluster_config_envoy_ptr: envoy_dynamic_module_type_cluster_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _label_names: *mut envoy_dynamic_module_type_module_buffer,
    _label_names_length: usize,
    _histogram_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_config_record_histogram_value(
    _cluster_config_envoy_ptr: envoy_dynamic_module_type_cluster_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_context_compute_hash_key(
    _context_envoy_ptr: envoy_dynamic_module_type_cluster_lb_context_envoy_ptr,
    _hash_out: *mut u64,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_context_get_downstream_headers_size(
    _context_envoy_ptr: envoy_dynamic_module_type_cluster_lb_context_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_context_get_downstream_headers(
    _context_envoy_ptr: envoy_dynamic_module_type_cluster_lb_context_envoy_ptr,
    _result_headers: *mut envoy_dynamic_module_type_envoy_http_header,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_context_get_downstream_header(
    _context_envoy_ptr: envoy_dynamic_module_type_cluster_lb_context_envoy_ptr,
    _key: envoy_dynamic_module_type_module_buffer,
    _result_buffer: *mut envoy_dynamic_module_type_envoy_buffer,
    _index: usize,
    _optional_size: *mut usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_context_get_host_selection_retry_count(
    _context_envoy_ptr: envoy_dynamic_module_type_cluster_lb_context_envoy_ptr,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_context_should_select_another_host(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _context_envoy_ptr: envoy_dynamic_module_type_cluster_lb_context_envoy_ptr,
    _priority: u32,
    _index: usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_context_get_override_host(
    _context_envoy_ptr: envoy_dynamic_module_type_cluster_lb_context_envoy_ptr,
    _address: *mut envoy_dynamic_module_type_envoy_buffer,
    _strict: *mut bool,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_context_get_downstream_connection_sni(
    _context_envoy_ptr: envoy_dynamic_module_type_cluster_lb_context_envoy_ptr,
    _result_buffer: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_async_host_selection_complete(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _context_envoy_ptr: envoy_dynamic_module_type_cluster_lb_context_envoy_ptr,
    _host: envoy_dynamic_module_type_cluster_host_envoy_ptr,
    _details: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_http_callout(
    _cluster_envoy_ptr: envoy_dynamic_module_type_cluster_envoy_ptr,
    _callout_id_out: *mut u64,
    _cluster_name: envoy_dynamic_module_type_module_buffer,
    _headers: *mut envoy_dynamic_module_type_module_http_header,
    _headers_size: usize,
    _body: envoy_dynamic_module_type_module_buffer,
    _timeout_milliseconds: u64,
) -> envoy_dynamic_module_type_http_callout_init_result {
    envoy_dynamic_module_type_http_callout_init_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cluster_lb_get_member_update_host_address(
    _lb_envoy_ptr: envoy_dynamic_module_type_cluster_lb_envoy_ptr,
    _index: usize,
    _is_added: bool,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_get_cluster_name(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_get_hosts_count(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _priority: u32,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_get_healthy_hosts_count(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _priority: u32,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_get_degraded_hosts_count(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _priority: u32,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_get_priority_set_size(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_get_healthy_host_address(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_get_healthy_host_weight(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_get_host_health(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
) -> envoy_dynamic_module_type_host_health {
    envoy_dynamic_module_type_host_health::Unhealthy
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_get_host_health_by_address(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _address: envoy_dynamic_module_type_module_buffer,
    _result: *mut envoy_dynamic_module_type_host_health,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_get_host_address(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_get_host_weight(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_get_host_locality(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
    _region: *mut envoy_dynamic_module_type_envoy_buffer,
    _zone: *mut envoy_dynamic_module_type_envoy_buffer,
    _sub_zone: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_set_host_data(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
    _data: usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_get_host_data(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
    _data: *mut usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_get_host_metadata_string(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
    _filter_name: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_get_host_metadata_number(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
    _filter_name: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _result: *mut f64,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_get_host_metadata_bool(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
    _filter_name: envoy_dynamic_module_type_module_buffer,
    _key: envoy_dynamic_module_type_module_buffer,
    _result: *mut bool,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_get_locality_count(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _priority: u32,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_get_locality_host_count(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _priority: u32,
    _locality_index: usize,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_get_locality_host_address(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _priority: u32,
    _locality_index: usize,
    _host_index: usize,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_get_locality_weight(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _priority: u32,
    _locality_index: usize,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_context_compute_hash_key(
    _context_envoy_ptr: envoy_dynamic_module_type_lb_context_envoy_ptr,
    _hash_out: *mut u64,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_context_get_downstream_headers_size(
    _context_envoy_ptr: envoy_dynamic_module_type_lb_context_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_context_get_downstream_headers(
    _context_envoy_ptr: envoy_dynamic_module_type_lb_context_envoy_ptr,
    _result_headers: *mut envoy_dynamic_module_type_envoy_http_header,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_context_get_downstream_header(
    _context_envoy_ptr: envoy_dynamic_module_type_lb_context_envoy_ptr,
    _key: envoy_dynamic_module_type_module_buffer,
    _result_buffer: *mut envoy_dynamic_module_type_envoy_buffer,
    _index: usize,
    _optional_size: *mut usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_context_get_host_selection_retry_count(
    _context_envoy_ptr: envoy_dynamic_module_type_lb_context_envoy_ptr,
) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_context_should_select_another_host(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _context_envoy_ptr: envoy_dynamic_module_type_lb_context_envoy_ptr,
    _priority: u32,
    _index: usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_context_get_override_host(
    _context_envoy_ptr: envoy_dynamic_module_type_lb_context_envoy_ptr,
    _address: *mut envoy_dynamic_module_type_envoy_buffer,
    _strict: *mut bool,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_get_member_update_host_address(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _index: usize,
    _is_added: bool,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_get_host_stat(
    _lb_envoy_ptr: envoy_dynamic_module_type_lb_envoy_ptr,
    _priority: u32,
    _index: usize,
    _stat: envoy_dynamic_module_type_host_stat,
) -> u64 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_config_define_counter(
    _lb_config_envoy_ptr: envoy_dynamic_module_type_lb_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _label_names: *mut envoy_dynamic_module_type_module_buffer,
    _label_names_length: usize,
    _counter_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_config_increment_counter(
    _lb_config_envoy_ptr: envoy_dynamic_module_type_lb_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_config_define_gauge(
    _lb_config_envoy_ptr: envoy_dynamic_module_type_lb_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _label_names: *mut envoy_dynamic_module_type_module_buffer,
    _label_names_length: usize,
    _gauge_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_config_set_gauge(
    _lb_config_envoy_ptr: envoy_dynamic_module_type_lb_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_config_increment_gauge(
    _lb_config_envoy_ptr: envoy_dynamic_module_type_lb_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_config_decrement_gauge(
    _lb_config_envoy_ptr: envoy_dynamic_module_type_lb_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_config_define_histogram(
    _lb_config_envoy_ptr: envoy_dynamic_module_type_lb_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _label_names: *mut envoy_dynamic_module_type_module_buffer,
    _label_names_length: usize,
    _histogram_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_lb_config_record_histogram_value(
    _lb_config_envoy_ptr: envoy_dynamic_module_type_lb_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_matcher_get_headers_size(
    _matcher_input_envoy_ptr: envoy_dynamic_module_type_matcher_input_envoy_ptr,
    _header_type: envoy_dynamic_module_type_http_header_type,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_matcher_get_headers(
    _matcher_input_envoy_ptr: envoy_dynamic_module_type_matcher_input_envoy_ptr,
    _header_type: envoy_dynamic_module_type_http_header_type,
    _result_headers: *mut envoy_dynamic_module_type_envoy_http_header,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_matcher_get_header_value(
    _matcher_input_envoy_ptr: envoy_dynamic_module_type_matcher_input_envoy_ptr,
    _header_type: envoy_dynamic_module_type_http_header_type,
    _key: envoy_dynamic_module_type_module_buffer,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
    _index: usize,
    _total_count_out: *mut usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cert_validator_set_error_details(
    _config_envoy_ptr: envoy_dynamic_module_type_cert_validator_config_envoy_ptr,
    _error_details: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cert_validator_set_filter_state(
    _config_envoy_ptr: envoy_dynamic_module_type_cert_validator_config_envoy_ptr,
    _key: envoy_dynamic_module_type_module_buffer,
    _value: envoy_dynamic_module_type_module_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_cert_validator_get_filter_state(
    _config_envoy_ptr: envoy_dynamic_module_type_cert_validator_config_envoy_ptr,
    _key: envoy_dynamic_module_type_module_buffer,
    _value_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_upstream_http_tcp_bridge_get_request_header(
    _bridge_envoy_ptr: envoy_dynamic_module_type_upstream_http_tcp_bridge_envoy_ptr,
    _key: envoy_dynamic_module_type_module_buffer,
    _result: *mut envoy_dynamic_module_type_envoy_buffer,
    _index: usize,
    _total_count_out: *mut usize,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_upstream_http_tcp_bridge_get_request_headers_size(
    _bridge_envoy_ptr: envoy_dynamic_module_type_upstream_http_tcp_bridge_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_upstream_http_tcp_bridge_get_request_headers(
    _bridge_envoy_ptr: envoy_dynamic_module_type_upstream_http_tcp_bridge_envoy_ptr,
    _result_headers: *mut envoy_dynamic_module_type_envoy_http_header,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_upstream_http_tcp_bridge_get_request_buffer(
    _bridge_envoy_ptr: envoy_dynamic_module_type_upstream_http_tcp_bridge_envoy_ptr,
    _result_buffer: *mut envoy_dynamic_module_type_envoy_buffer,
    _result_buffer_length: *mut usize,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_upstream_http_tcp_bridge_get_response_buffer(
    _bridge_envoy_ptr: envoy_dynamic_module_type_upstream_http_tcp_bridge_envoy_ptr,
    _result_buffer: *mut envoy_dynamic_module_type_envoy_buffer,
    _result_buffer_length: *mut usize,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_upstream_http_tcp_bridge_send_upstream_data(
    _bridge_envoy_ptr: envoy_dynamic_module_type_upstream_http_tcp_bridge_envoy_ptr,
    _data: envoy_dynamic_module_type_module_buffer,
    _end_stream: bool,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_upstream_http_tcp_bridge_send_response(
    _bridge_envoy_ptr: envoy_dynamic_module_type_upstream_http_tcp_bridge_envoy_ptr,
    _status_code: u32,
    _headers_vector: *mut envoy_dynamic_module_type_module_http_header,
    _headers_vector_size: usize,
    _body: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_upstream_http_tcp_bridge_send_response_headers(
    _bridge_envoy_ptr: envoy_dynamic_module_type_upstream_http_tcp_bridge_envoy_ptr,
    _status_code: u32,
    _headers_vector: *mut envoy_dynamic_module_type_module_http_header,
    _headers_vector_size: usize,
    _end_stream: bool,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_upstream_http_tcp_bridge_send_response_data(
    _bridge_envoy_ptr: envoy_dynamic_module_type_upstream_http_tcp_bridge_envoy_ptr,
    _data: envoy_dynamic_module_type_module_buffer,
    _end_stream: bool,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_upstream_http_tcp_bridge_send_response_trailers(
    _bridge_envoy_ptr: envoy_dynamic_module_type_upstream_http_tcp_bridge_envoy_ptr,
    _trailers_vector: *mut envoy_dynamic_module_type_module_http_header,
    _trailers_vector_size: usize,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_tracer_get_trace_context_value(
    _span_envoy_ptr: envoy_dynamic_module_type_tracer_span_envoy_ptr,
    _key: envoy_dynamic_module_type_module_buffer,
    _value_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_tracer_set_trace_context_value(
    _span_envoy_ptr: envoy_dynamic_module_type_tracer_span_envoy_ptr,
    _key: envoy_dynamic_module_type_module_buffer,
    _value: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_tracer_remove_trace_context_value(
    _span_envoy_ptr: envoy_dynamic_module_type_tracer_span_envoy_ptr,
    _key: envoy_dynamic_module_type_module_buffer,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_tracer_get_trace_context_protocol(
    _span_envoy_ptr: envoy_dynamic_module_type_tracer_span_envoy_ptr,
    _value_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_tracer_get_trace_context_host(
    _span_envoy_ptr: envoy_dynamic_module_type_tracer_span_envoy_ptr,
    _value_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_tracer_get_trace_context_path(
    _span_envoy_ptr: envoy_dynamic_module_type_tracer_span_envoy_ptr,
    _value_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_tracer_get_trace_context_method(
    _span_envoy_ptr: envoy_dynamic_module_type_tracer_span_envoy_ptr,
    _value_out: *mut envoy_dynamic_module_type_envoy_buffer,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_tracer_define_counter(
    _config_envoy_ptr: envoy_dynamic_module_type_tracer_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _label_names: *mut envoy_dynamic_module_type_module_buffer,
    _label_names_length: usize,
    _counter_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_tracer_define_gauge(
    _config_envoy_ptr: envoy_dynamic_module_type_tracer_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _label_names: *mut envoy_dynamic_module_type_module_buffer,
    _label_names_length: usize,
    _gauge_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_tracer_define_histogram(
    _config_envoy_ptr: envoy_dynamic_module_type_tracer_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _label_names: *mut envoy_dynamic_module_type_module_buffer,
    _label_names_length: usize,
    _histogram_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_tracer_increment_counter(
    _config_envoy_ptr: envoy_dynamic_module_type_tracer_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_tracer_record_histogram_value(
    _config_envoy_ptr: envoy_dynamic_module_type_tracer_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_tracer_set_gauge(
    _config_envoy_ptr: envoy_dynamic_module_type_tracer_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_dns_resolve_complete(
    _resolver_envoy_ptr: envoy_dynamic_module_type_dns_resolver_envoy_ptr,
    _query_id: u64,
    _status: envoy_dynamic_module_type_dns_resolution_status,
    _details: envoy_dynamic_module_type_module_buffer,
    _addresses: *const envoy_dynamic_module_type_dns_address,
    _num_addresses: usize,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_dns_resolver_config_define_counter(
    _config_envoy_ptr: envoy_dynamic_module_type_dns_resolver_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _label_names: *mut envoy_dynamic_module_type_module_buffer,
    _label_names_length: usize,
    _counter_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_dns_resolver_config_increment_counter(
    _config_envoy_ptr: envoy_dynamic_module_type_dns_resolver_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_dns_resolver_config_define_gauge(
    _config_envoy_ptr: envoy_dynamic_module_type_dns_resolver_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _label_names: *mut envoy_dynamic_module_type_module_buffer,
    _label_names_length: usize,
    _gauge_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_dns_resolver_config_set_gauge(
    _config_envoy_ptr: envoy_dynamic_module_type_dns_resolver_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_dns_resolver_config_increment_gauge(
    _config_envoy_ptr: envoy_dynamic_module_type_dns_resolver_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_dns_resolver_config_decrement_gauge(
    _config_envoy_ptr: envoy_dynamic_module_type_dns_resolver_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_dns_resolver_config_define_histogram(
    _config_envoy_ptr: envoy_dynamic_module_type_dns_resolver_config_envoy_ptr,
    _name: envoy_dynamic_module_type_module_buffer,
    _label_names: *mut envoy_dynamic_module_type_module_buffer,
    _label_names_length: usize,
    _histogram_id_ptr: *mut usize,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_dns_resolver_config_record_histogram_value(
    _config_envoy_ptr: envoy_dynamic_module_type_dns_resolver_config_envoy_ptr,
    _id: usize,
    _label_values: *mut envoy_dynamic_module_type_module_buffer,
    _label_values_length: usize,
    _value: u64,
) -> envoy_dynamic_module_type_metrics_result {
    envoy_dynamic_module_type_metrics_result::Success
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_transport_socket_get_io_handle(
    _transport_socket_envoy_ptr: envoy_dynamic_module_type_transport_socket_envoy_ptr,
) -> *mut ::std::os::raw::c_void {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_transport_socket_io_handle_read(
    _io_handle: *mut ::std::os::raw::c_void,
    _buffer: *mut ::std::os::raw::c_char,
    _length: usize,
    _bytes_read: *mut usize,
) -> i64 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_transport_socket_io_handle_write(
    _io_handle: *mut ::std::os::raw::c_void,
    _buffer: *const ::std::os::raw::c_char,
    _length: usize,
    _bytes_written: *mut usize,
) -> i64 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_transport_socket_io_handle_fd(
    _io_handle: *mut ::std::os::raw::c_void,
) -> ::std::os::raw::c_int {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_transport_socket_read_buffer_drain(
    _transport_socket_envoy_ptr: envoy_dynamic_module_type_transport_socket_envoy_ptr,
    _length: usize,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_transport_socket_read_buffer_add(
    _transport_socket_envoy_ptr: envoy_dynamic_module_type_transport_socket_envoy_ptr,
    _data: *const ::std::os::raw::c_char,
    _length: usize,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_transport_socket_read_buffer_length(
    _transport_socket_envoy_ptr: envoy_dynamic_module_type_transport_socket_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_transport_socket_write_buffer_drain(
    _transport_socket_envoy_ptr: envoy_dynamic_module_type_transport_socket_envoy_ptr,
    _length: usize,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_transport_socket_write_buffer_get_slices(
    _transport_socket_envoy_ptr: envoy_dynamic_module_type_transport_socket_envoy_ptr,
    _slices: *mut envoy_dynamic_module_type_envoy_buffer,
    _slices_count: *mut usize,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_transport_socket_write_buffer_length(
    _transport_socket_envoy_ptr: envoy_dynamic_module_type_transport_socket_envoy_ptr,
) -> usize {
    0
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_transport_socket_raise_event(
    _transport_socket_envoy_ptr: envoy_dynamic_module_type_transport_socket_envoy_ptr,
    _event: envoy_dynamic_module_type_network_connection_event,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_transport_socket_should_drain_read_buffer(
    _transport_socket_envoy_ptr: envoy_dynamic_module_type_transport_socket_envoy_ptr,
) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_transport_socket_set_is_readable(
    _transport_socket_envoy_ptr: envoy_dynamic_module_type_transport_socket_envoy_ptr,
) {
    // no-op
}

#[no_mangle]
pub unsafe extern "C" fn envoy_dynamic_module_callback_transport_socket_flush_write_buffer(
    _transport_socket_envoy_ptr: envoy_dynamic_module_type_transport_socket_envoy_ptr,
) {
    // no-op
}
