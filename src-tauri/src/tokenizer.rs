// builds the 3 character trigram
// if the len < 3, we'll jsut return the entire string 
pub fn build_trigrams(s: &str) -> String {

let len = s.len();

if len < 3 {
    return s.to_string();
}

// for length >= 3, we produce overlapping tokens
// i.e. for "tokens" -> "tok", "oke", "ken", "ens"
let mut tokens = Vec::with_capacity(len-2);
// subtract 2 to determine the total number of tokens to output

for i in 0..(len - 2){
    tokens.push(&s[i..i + 3]);
}
// join with spaces so FTS sees each 3-char slice as a separate token
tokens.join(" ")
}

// combine name/path/extension trigrams into one doc_text string that fs5 can search over
pub fn build_doc_text(name: &str, path: &str, extension: &str) -> String {

    let mut parts = Vec::new();
    parts.push(build_trigrams(name));
    parts.push(build_trigrams(path));
    parts.push(build_trigrams(extension));

    println!("the tokens: {:?}", parts);

    parts.join(" ")
}