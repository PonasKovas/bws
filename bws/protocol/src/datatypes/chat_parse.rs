use super::*;

pub fn parse<T: AsRef<str>>(input: T) -> Chat {
    let mut result = Chat::new();
    let mut innermost = &mut result;
    let mut escaping = false;
    let mut modifying = false;

    for character in input.as_ref().chars() {
        if !escaping && !modifying {
            if character == '\\' {
                escaping = true;
            } else if character == '§' {
                modifying = true;
            } else {
                innermost.text.push(character);
            }
        } else {
            if escaping {
                innermost.text.push(character);
                escaping = false;
            } else if modifying {
                if innermost.text.len() > 0 {
                    innermost.extra.push(Chat::new());
                    innermost = &mut innermost.extra[0];
                }

                match character {
                    'l' => {
                        innermost.bold = Some(true);
                    }
                    'k' => {
                        innermost.obfuscated = Some(true);
                    }
                    'm' => {
                        innermost.strikethrough = Some(true);
                    }
                    'n' => {
                        innermost.underlined = Some(true);
                    }
                    'o' => {
                        innermost.italic = Some(true);
                    }
                    'r' => {
                        innermost.bold = Some(false);
                        innermost.obfuscated = Some(false);
                        innermost.strikethrough = Some(false);
                        innermost.underlined = Some(false);
                        innermost.italic = Some(false);
                        innermost.color = Some("reset".to_string());
                    }
                    '0' => {
                        innermost.color = Some("black".to_string());
                    }
                    '1' => {
                        innermost.color = Some("dark_blue".to_string());
                    }
                    '2' => {
                        innermost.color = Some("dark_green".to_string());
                    }
                    '3' => {
                        innermost.color = Some("dark_aqua".to_string());
                    }
                    '4' => {
                        innermost.color = Some("dark_red".to_string());
                    }
                    '5' => {
                        innermost.color = Some("dark_purple".to_string());
                    }
                    '6' => {
                        innermost.color = Some("gold".to_string());
                    }
                    '7' => {
                        innermost.color = Some("gray".to_string());
                    }
                    '8' => {
                        innermost.color = Some("dark_gray".to_string());
                    }
                    '9' => {
                        innermost.color = Some("blue".to_string());
                    }
                    'a' => {
                        innermost.color = Some("green".to_string());
                    }
                    'b' => {
                        innermost.color = Some("aqua".to_string());
                    }
                    'c' => {
                        innermost.color = Some("red".to_string());
                    }
                    'd' => {
                        innermost.color = Some("light_purple".to_string());
                    }
                    'e' => {
                        innermost.color = Some("yellow".to_string());
                    }
                    'f' => {
                        innermost.color = Some("white".to_string());
                    }
                    _ => {}
                }
                modifying = false;
            }
        }
    }

    result
}
