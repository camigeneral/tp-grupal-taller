pub fn decode_text(text: String) -> String {
    let new_text = String::from(text);
    new_text.replace("<space>", " ")
    .replace("<enter>", "\n")
    .replace("<delete>", "")
}