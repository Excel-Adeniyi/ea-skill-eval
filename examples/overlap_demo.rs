fn main() {
    let question = "Explain how Rust ownership works";
    let answer = "Rust ownership means each value has a single owner. \
                  When the owner goes out of scope the value is dropped.";

    let q = ea_skill_eval::scoring::extract_terms(question);
    let a = ea_skill_eval::scoring::extract_terms(answer);

    let mut qs: Vec<_> = q.iter().cloned().collect();
    qs.sort();
    let mut as_: Vec<_> = a.iter().cloned().collect();
    as_.sort();
    let mut shared: Vec<_> = q.intersection(&a).cloned().collect();
    shared.sort();

    println!("question words : {qs:?}");
    println!("answer words   : {as_:?}");
    println!("shared         : {shared:?}");
    println!(
        "score          : {} shared / {} question words = {:.2}",
        shared.len(),
        q.len(),
        shared.len() as f64 / q.len() as f64
    );
}
