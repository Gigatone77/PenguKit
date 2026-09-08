use pengu_archive::Archive;
fn main() {
    let ar = Archive::open(std::env::args().nth(1).unwrap()).unwrap();
    let idx = ar.read_index().unwrap();
    for (i, f) in idx.files.iter().enumerate() {
        for k in f.segments_start..f.segments_end {
            eprintln!("{}: seg {}", i, k);
            let _ = ar.read_segment(&idx.segments[k as usize]);
        }
    }
    eprintln!("all ok");
}
