use rusqlite::vtab::{
    sqlite3_tokenizer_module, Context, CreateTokenizerModule, Tokenizer, TokenizerModule,
    TokenizerControl,
};
use rusqlite::{Connection, Result};

pub struct TrigramTokenizerModule;

// create an instance of the tokenizer
impl TokenizerModule for TrigramTokenizerModule {
    type Tokenizer = TrigramTokenizer;

    fn create(&self, _args: &[&[u8]]) -> Result<Self::Tokenizer> {
        Ok(TrigramTokenizer)
    }
}

pub struct TrigramTokenizer;

/// break input text into trigram tokens
impl Tokenizer for TrigramTokenizer {
    fn tokenize<F>(&self, text: &str, mut token_callback: F) -> Result<()>
    where
        F: FnMut(&str, usize, usize, TokenizerControl) -> Result<()>,
    {
        // We'll produce overlapping 3-character substrings.
        // For example, "example.pdf" -> "exa", "xam", "amp", "mpl", "ple", "le.", "e.p", ".pd", "pdf"
        let length = text.len();
        if length < 3 {
            // If the string is too short, you may want to treat it as a single token or skip.
            // We'll treat 1-2 character strings as single tokens:
            return token_callback(text, 0, length, TokenizerControl::empty());
        }

        // Generate every 3-character substring
        for i in 0..(length - 2) {
            let token = &text[i..i + 3];
            // The (start, end) offsets in the original text:
            let start_offset = i;
            let end_offset = i + 3;

            token_callback(token, start_offset, end_offset, TokenizerControl::empty())?;
        }

        Ok(())
    }
}

/// A convenience function to register the `trigram` tokenizer module with a SQLite connection.
/// Must be called *before* creating the FTS table that uses `tokenize='trigram'`.
pub fn init_trigram_tokenizer_module(conn: &Connection) -> Result<()> {
    let tokenizer_module = CreateTokenizerModule::new(TrigramTokenizerModule);
    // Register our module name as "trigram"
    conn.create_module("trigram", tokenizer_module)
}
