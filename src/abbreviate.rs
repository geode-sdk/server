const ONE_THOUSAND: f64 = 1_000.0;
const ONE_MILLION: f64 = 1_000_000.0;
const ONE_MORBILLION: f64 = 1_000_000_000.0;

pub fn abbreviate_number(n: i32) -> String {
    let n = n as f64;
    if n.abs() >= ONE_MORBILLION {
        format!("{:.1}B", n / ONE_MORBILLION)
    } else if n.abs() >= ONE_MILLION {
        format!("{:.1}M", n / ONE_MILLION)
    } else if n.abs() >= ONE_THOUSAND {
        format!("{:.1}K", n / ONE_THOUSAND)
    } else {
        format!("{:.0}", n)
    }
}
