use text_io::read;


fn main() {
    loop {
        let line: String = read!("{}\n");
        println!("Hulk {}", line.to_uppercase());

    }
}
