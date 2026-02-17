use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Expr {
    Literal(Literal),
    Field {
        entity: String,
        field: String,
    },
    Op {
        op: OpKind,
        args: Vec<Expr>,
    },
    Quantifier {
        kind: QuantifierKind,
        var: String,
        domain: String,
        body: Box<Expr>,
    },
    FnCall {
        classification: FnClassification,
        name: String,
        args: Vec<String>,
    },
    Is {
        entity: String,
        refinement: String,
        params: std::collections::HashMap<String, String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Literal {
    Bool(bool),
    Int(i64),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpKind {
    Eq,
    Neq,
    And,
    Or,
    Not,
    Implies,
    Lt,
    Lte,
    Gt,
    Gte,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantifierKind {
    Forall,
    Exists,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FnClassification {
    Derived,
    Observer,
}

impl<'de> Deserialize<'de> for Expr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        parse_expr(&value).map_err(serde::de::Error::custom)
    }
}

fn parse_expr(value: &serde_json::Value) -> Result<Expr, String> {
    match value {
        // Literals: bool, number, string (non-array)
        serde_json::Value::Bool(b) => Ok(Expr::Literal(Literal::Bool(*b))),
        serde_json::Value::Number(n) => {
            let i = n.as_i64().ok_or_else(|| format!("unsupported number: {n}"))?;
            Ok(Expr::Literal(Literal::Int(i)))
        }
        serde_json::Value::String(s) => Ok(Expr::Literal(Literal::String(s.clone()))),

        // Array forms: ["op", ...args]
        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                return Err("empty expression array".to_string());
            }
            let tag = arr[0]
                .as_str()
                .ok_or_else(|| format!("first element of expression array must be a string, got: {:?}", arr[0]))?;

            match tag {
                // Field access: ["field", entity, field_name]
                "field" => {
                    if arr.len() != 3 {
                        return Err(format!("field expression requires 3 elements, got {}", arr.len()));
                    }
                    let entity = arr[1].as_str().ok_or("field entity must be a string")?.to_string();
                    let field = arr[2].as_str().ok_or("field name must be a string")?.to_string();
                    Ok(Expr::Field { entity, field })
                }

                // Quantifiers: ["forall"|"exists", var, domain, body]
                "forall" | "exists" => {
                    if arr.len() != 4 {
                        return Err(format!("{tag} expression requires 4 elements, got {}", arr.len()));
                    }
                    let kind = match tag {
                        "forall" => QuantifierKind::Forall,
                        "exists" => QuantifierKind::Exists,
                        _ => unreachable!(),
                    };
                    let var = arr[1].as_str().ok_or("quantifier var must be a string")?.to_string();
                    let domain = arr[2].as_str().ok_or("quantifier domain must be a string")?.to_string();
                    let body = Box::new(parse_expr(&arr[3])?);
                    Ok(Expr::Quantifier { kind, var, domain, body })
                }

                // Function calls: ["derived"|"observer", name, ...args]
                "derived" | "observer" => {
                    if arr.len() < 3 {
                        return Err(format!("{tag} expression requires at least 3 elements, got {}", arr.len()));
                    }
                    let classification = match tag {
                        "derived" => FnClassification::Derived,
                        "observer" => FnClassification::Observer,
                        _ => unreachable!(),
                    };
                    let name = arr[1].as_str().ok_or("function name must be a string")?.to_string();
                    let args = arr[2..]
                        .iter()
                        .map(|v| v.as_str().ok_or("function arg must be a string").map(|s| s.to_string()))
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(Expr::FnCall { classification, name, args })
                }

                // Is expression: ["is", entity, refinement, {params}]
                "is" => {
                    if arr.len() < 3 || arr.len() > 4 {
                        return Err(format!("is expression requires 3-4 elements, got {}", arr.len()));
                    }
                    let entity = arr[1].as_str().ok_or("is entity must be a string")?.to_string();
                    let refinement = arr[2].as_str().ok_or("is refinement must be a string")?.to_string();
                    let params = if arr.len() == 4 {
                        let obj = arr[3].as_object().ok_or("is params must be an object")?;
                        obj.iter()
                            .map(|(k, v)| {
                                let val = v.as_str().ok_or("is param value must be a string")?;
                                Ok((k.clone(), val.to_string()))
                            })
                            .collect::<Result<std::collections::HashMap<_, _>, &str>>()
                            .map_err(|e| e.to_string())?
                    } else {
                        std::collections::HashMap::new()
                    };
                    Ok(Expr::Is { entity, refinement, params })
                }

                // Operators: ["eq"|"neq"|"and"|"or"|"not"|"implies"|"lt"|"lte"|"gt"|"gte", ...args]
                _ => {
                    let op = match tag {
                        "eq" => OpKind::Eq,
                        "neq" => OpKind::Neq,
                        "and" => OpKind::And,
                        "or" => OpKind::Or,
                        "not" => OpKind::Not,
                        "implies" => OpKind::Implies,
                        "lt" => OpKind::Lt,
                        "lte" => OpKind::Lte,
                        "gt" => OpKind::Gt,
                        "gte" => OpKind::Gte,
                        other => return Err(format!("unknown expression operator: {other}")),
                    };
                    let args = arr[1..]
                        .iter()
                        .map(parse_expr)
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(Expr::Op { op, args })
                }
            }
        }

        other => Err(format!("unsupported expression value: {other}")),
    }
}
