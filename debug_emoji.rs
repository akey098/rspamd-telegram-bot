fn main() {
    let text = "Hello! 😀😃😄😁😆😅😂🤣😊😇🙂🙃";
    let emoji_count = text.chars().filter(|c| {
        let code = *c as u32;
        (code >= 0x1F600 && code <= 0x1F64F) || // emoticons
        (code >= 0x1F300 && code <= 0x1F5FF) || // misc symbols
        (code >= 0x1F680 && code <= 0x1F6FF) || // transport
        (code >= 0x2600 && code <= 0x26FF) ||   // misc symbols
        (code >= 0x2700 && code <= 0x27BF)      // dingbats
    }).count();
    
    println!("Text: {}", text);
    println!("Emoji count: {}", emoji_count);
    
    for c in text.chars() {
        let code = c as u32;
        if (code >= 0x1F600 && code <= 0x1F64F) ||
           (code >= 0x1F300 && code <= 0x1F5FF) ||
           (code >= 0x1F680 && code <= 0x1F6FF) ||
           (code >= 0x2600 && code <= 0x26FF) ||
           (code >= 0x2700 && code <= 0x27BF) {
            println!("Emoji: {} (U+{:04X})", c, code);
        }
    }
}
