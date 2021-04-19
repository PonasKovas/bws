use serde_json::{json, Value};

// Converts a string with § to a json object that is used internally in minecraft
pub fn parse(mut input: String) -> Value {
    let mut modifiers = Vec::new();

    let mut i = 0;
    while i < input.chars().count() {
        // find all §
        if input.chars().nth(i).unwrap() == '§' {
            // check if it was escaped
            // (cant be escaped if its the first char)
            if i > 0 {
                if input.chars().nth(i - 1).unwrap() == '\\' {
                    // remove the escape character
                    let byte_index = input.char_indices().nth(i - 1).unwrap().0;
                    input.remove(byte_index);
                    continue;
                }
                // or if its not in valid format
                if input.chars().nth(i + 1).is_none() {
                    // there is no modifier following
                    // so just ignore this whole thing
                    i += 1;
                    continue;
                }
            }

            // save the modifier
            let modifier = input.chars().nth(i + 1).unwrap();
            modifiers.push((i, modifier));

            // remove the characters
            let start_byte_index = input.char_indices().nth(i).unwrap().0;
            let end_byte_index = input.char_indices().nth(i + 2).unwrap().0;
            input.replace_range(start_byte_index..end_byte_index, "");
            continue;
        }
        // normal text
        i += 1;
    }

    let mut result = json!({});

    if modifiers.len() == 0 {
        // if there are no modifiers just add the normal text
        result["text"] = json!(input);
    } else {
        // add the beggining text without any modifiers if there is any
        result["text"] = json!(input.chars().take(modifiers[0].0).collect::<String>());
    }

    let mut last_pos = 0;
    let mut depth = 0;
    for (i, modifier) in modifiers.iter().enumerate() {
        if modifier.0 != last_pos {
            depth += 1;
        }

        let mut container = &mut result;
        for _ in 0..depth {
            if !container["extra"].is_array() {
                container["extra"] = json!([{}]);
            }
            container = &mut container["extra"][0];
        }

        match modifier.1 {
            'l' => {
                container["bold"] = json!(true);
            }
            'k' => {
                container["obfuscated"] = json!(true);
            }
            'm' => {
                container["strikethrough"] = json!(true);
            }
            'n' => {
                container["underlined"] = json!(true);
            }
            'o' => {
                container["italic"] = json!(true);
            }
            'r' => {
                container["bold"] = json!(false);
                container["obfuscated"] = json!(false);
                container["strikethrough"] = json!(false);
                container["underlined"] = json!(false);
                container["italic"] = json!(false);
                container["color"] = json!("reset");
            }
            '0' => {
                container["color"] = json!("black");
            }
            '1' => {
                container["color"] = json!("dark_blue");
            }
            '2' => {
                container["color"] = json!("dark_green");
            }
            '3' => {
                container["color"] = json!("dark_aqua");
            }
            '4' => {
                container["color"] = json!("dark_red");
            }
            '5' => {
                container["color"] = json!("dark_purple");
            }
            '6' => {
                container["color"] = json!("gold");
            }
            '7' => {
                container["color"] = json!("gray");
            }
            '8' => {
                container["color"] = json!("dark_gray");
            }
            '9' => {
                container["color"] = json!("blue");
            }
            'a' => {
                container["color"] = json!("green");
            }
            'b' => {
                container["color"] = json!("aqua");
            }
            'c' => {
                container["color"] = json!("red");
            }
            'd' => {
                container["color"] = json!("light_purple");
            }
            'e' => {
                container["color"] = json!("yellow");
            }
            'f' => {
                container["color"] = json!("white");
            }
            _ => {}
        }

        if i == modifiers.len() - 1 {
            // the last segment so just read to end
            let lower_bound = input.char_indices().nth(modifier.0).unwrap().0;
            container["text"] = json!(input[lower_bound..]);
        } else {
            // if last modifier of the same segment
            if modifiers[i + 1].0 != modifier.0 {
                // (different position follows)
                // add the text
                let lower_bound = input.char_indices().nth(modifier.0).unwrap().0;
                let upper_bound = input.char_indices().nth(modifiers[i + 1].0).unwrap().0;
                container["text"] = json!(input[lower_bound..upper_bound]);
            }
        }

        last_pos = modifier.0;
    }

    result
}
