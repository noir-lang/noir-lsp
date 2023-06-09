use std::collections::HashMap;


use noir_language_server::chumsky::{parse, type_inference};

fn main() {
    let source = include_str!("./test.nr");
    // let source = r#"
    // test
    // println!("{:?}", &source[10..11]);
    let (ast, errors, _semantic_tokens) = parse(source);
    println!("{:?}", errors);
    // if let Some(ref ast) = ast {
    //     println!("{:#?}", ast);
    // } else {
    //     println!("{:?}", errors);
    // }
    // println!("{:?}", semantic_tokens);
    let mut hashmap = HashMap::new();
    if let Some(ast) = ast {
        ast.into_iter().for_each(|(_k, v)| {
            type_inference(&v.body, &mut hashmap);
        });
    }
    println!("{:?}", hashmap);
}
