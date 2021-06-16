use super::*;

pub fn parse<'a, T: AsRef<str>>(input: T) -> Chat<'a> {
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
                innermost.text.to_mut().push(character);
            }
        } else {
            if escaping {
                innermost.text.to_mut().push(character);
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
                        innermost.color = Some("reset".into());
                    }
                    '0' => {
                        innermost.color = Some("black".into());
                    }
                    '1' => {
                        innermost.color = Some("dark_blue".into());
                    }
                    '2' => {
                        innermost.color = Some("dark_green".into());
                    }
                    '3' => {
                        innermost.color = Some("dark_aqua".into());
                    }
                    '4' => {
                        innermost.color = Some("dark_red".into());
                    }
                    '5' => {
                        innermost.color = Some("dark_purple".into());
                    }
                    '6' => {
                        innermost.color = Some("gold".into());
                    }
                    '7' => {
                        innermost.color = Some("gray".into());
                    }
                    '8' => {
                        innermost.color = Some("dark_gray".into());
                    }
                    '9' => {
                        innermost.color = Some("blue".into());
                    }
                    'a' => {
                        innermost.color = Some("green".into());
                    }
                    'b' => {
                        innermost.color = Some("aqua".into());
                    }
                    'c' => {
                        innermost.color = Some("red".into());
                    }
                    'd' => {
                        innermost.color = Some("light_purple".into());
                    }
                    'e' => {
                        innermost.color = Some("yellow".into());
                    }
                    'f' => {
                        innermost.color = Some("white".into());
                    }
                    _ => {}
                }
                modifying = false;
            }
        }
    }

    result
}
