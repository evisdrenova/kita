use std::time::Instant;

pub fn get_code_execution_duration() {

    let path_start = Instant::now();
let path_str: id = msg_send![class!(NSString), 
    stringWithUTF8String: app_path.as_ptr()];
println!("Path string creation took: {:?}", path_start.elapsed());



}