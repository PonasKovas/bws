use crate::datatypes::*;
use anyhow::Result;
use log::debug;
use serde_json::{json, to_string, Value};

pub fn parse<T: AsRef<str>>(input: T) -> Chat {
    Chat(to_string(&parse_json(input)).unwrap())
}

// Converts a string with § to a json object that is used internally in minecraft
pub fn parse_json<T: AsRef<str>>(input: T) -> Value {
    let mut result = json!({});
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
                if let Some(Value::String(s)) = innermost.get_mut("text") {
                    s.push(character);
                } else {
                    innermost["text"] = json!(character.to_string());
                }
            }
        } else {
            if escaping {
                if let Some(Value::String(s)) = innermost.get_mut("text") {
                    s.push(character);
                } else {
                    innermost["text"] = json!(character.to_string());
                }
                escaping = false;
            } else if modifying {
                if innermost.get("text").is_some() {
                    innermost["extra"] = json!([{}]);
                    innermost = &mut innermost["extra"][0];
                }

                match character {
                    'l' => {
                        innermost["bold"] = json!(true);
                    }
                    'k' => {
                        innermost["obfuscated"] = json!(true);
                    }
                    'm' => {
                        innermost["strikethrough"] = json!(true);
                    }
                    'n' => {
                        innermost["underlined"] = json!(true);
                    }
                    'o' => {
                        innermost["italic"] = json!(true);
                    }
                    'r' => {
                        innermost["bold"] = json!(false);
                        innermost["obfuscated"] = json!(false);
                        innermost["strikethrough"] = json!(false);
                        innermost["underlined"] = json!(false);
                        innermost["italic"] = json!(false);
                        innermost["color"] = json!("reset");
                    }
                    '0' => {
                        innermost["color"] = json!("black");
                    }
                    '1' => {
                        innermost["color"] = json!("dark_blue");
                    }
                    '2' => {
                        innermost["color"] = json!("dark_green");
                    }
                    '3' => {
                        innermost["color"] = json!("dark_aqua");
                    }
                    '4' => {
                        innermost["color"] = json!("dark_red");
                    }
                    '5' => {
                        innermost["color"] = json!("dark_purple");
                    }
                    '6' => {
                        innermost["color"] = json!("gold");
                    }
                    '7' => {
                        innermost["color"] = json!("gray");
                    }
                    '8' => {
                        innermost["color"] = json!("dark_gray");
                    }
                    '9' => {
                        innermost["color"] = json!("blue");
                    }
                    'a' => {
                        innermost["color"] = json!("green");
                    }
                    'b' => {
                        innermost["color"] = json!("aqua");
                    }
                    'c' => {
                        innermost["color"] = json!("red");
                    }
                    'd' => {
                        innermost["color"] = json!("light_purple");
                    }
                    'e' => {
                        innermost["color"] = json!("yellow");
                    }
                    'f' => {
                        innermost["color"] = json!("white");
                    }
                    _ => {}
                }
                modifying = false;
            }
        }
    }

    if result.get("text").is_none() {
        result["text"] = json!("");
    }

    result
}
