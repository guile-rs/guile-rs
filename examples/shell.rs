extern crate guile;

fn main() {
    guile::init(|vm| {
        let args = vec!["Test".to_string()];
        vm.shell(args);
    });
}
