#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NaMoveDirection {
    Forward,
    Backward,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NaMoveBinding {
    pub key: String,
    pub direction: NaMoveDirection,
    pub speed: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NaScriptProgram {
    pub entity_name: Option<String>,
    pub attach_player_camera: bool,
    pub move_bindings: Vec<NaMoveBinding>,
}

pub fn parse_na_script(source: &str) -> Result<NaScriptProgram, String> {
    let mut program = NaScriptProgram {
        entity_name: None,
        attach_player_camera: false,
        move_bindings: Vec::new(),
    };
    let mut pending_key: Option<String> = None;

    for raw_line in source.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }

        if let Some(rest) = line.strip_prefix("entity ") {
            if let Some(name) = rest.split_whitespace().next() {
                program.entity_name = Some(name.trim_end_matches('{').to_string());
            }
            continue;
        }

        if line == "player_camera.attach(self);" {
            program.attach_player_camera = true;
            continue;
        }

        if let Some(key) = parse_input_key_condition(line) {
            pending_key = Some(key);
            continue;
        }

        if let Some(key) = pending_key.clone() {
            if let Some((direction, speed)) = parse_move_call(line)? {
                program.move_bindings.push(NaMoveBinding {
                    key,
                    direction,
                    speed,
                });
                pending_key = None;
                continue;
            }
        }

        if line == "}" {
            pending_key = None;
        }
    }

    Ok(program)
}

fn parse_input_key_condition(line: &str) -> Option<String> {
    let prefix = "if input.key_down(\"";
    let rest = line.strip_prefix(prefix)?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn parse_move_call(line: &str) -> Result<Option<(NaMoveDirection, f32)>, String> {
    let candidates = [
        ("self.move_forward(", NaMoveDirection::Forward),
        ("self.move_backward(", NaMoveDirection::Backward),
        ("self.move_left(", NaMoveDirection::Left),
        ("self.move_right(", NaMoveDirection::Right),
    ];
    for (prefix, direction) in candidates {
        let Some(rest) = line.strip_prefix(prefix) else {
            continue;
        };
        let Some(end) = rest.find(')') else {
            return Err(format!("invalid move call `{line}`"));
        };
        let expr = rest[..end].trim();
        let speed = parse_speed_expression(expr)?;
        return Ok(Some((direction, speed)));
    }
    Ok(None)
}

fn parse_speed_expression(expr: &str) -> Result<f32, String> {
    if let Some((lhs, rhs)) = expr.split_once('*') {
        let left = lhs.trim();
        let right = rhs.trim();
        if right == "delta_time" {
            return left
                .parse::<f32>()
                .map_err(|_| format!("invalid movement speed `{expr}`"));
        }
        if left == "delta_time" {
            return right
                .parse::<f32>()
                .map_err(|_| format!("invalid movement speed `{expr}`"));
        }
    }
    expr.parse::<f32>()
        .map_err(|_| format!("invalid movement speed `{expr}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_extracts_camera_attach_and_move_bindings() {
        let program = parse_na_script(
            r#"
entity CarController {
    on_start() {
        player_camera.attach(self);
    }

    on_update(delta_time) {
        if input.key_down("W") {
            self.move_forward(4.5 * delta_time);
        }
        if input.key_down("A") {
            self.move_left(2.0 * delta_time);
        }
    }
}
"#,
        )
        .expect("script should parse");

        assert_eq!(program.entity_name.as_deref(), Some("CarController"));
        assert!(program.attach_player_camera);
        assert_eq!(program.move_bindings.len(), 2);
        assert_eq!(program.move_bindings[0].key, "W");
        assert_eq!(program.move_bindings[0].direction, NaMoveDirection::Forward);
        assert!((program.move_bindings[0].speed - 4.5).abs() < 0.001);
        assert_eq!(program.move_bindings[1].direction, NaMoveDirection::Left);
    }
}
