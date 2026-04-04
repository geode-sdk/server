const ONE_THOUSAND: f64 = 1_000.0;
const ONE_MILLION: f64 = 1_000_000.0;
const ONE_BILLION: f64 = 1_000_000_000.0;

pub fn abbreviate_number(n: i32) -> String {
    let n = n as f64;
    if n.abs() >= ONE_BILLION {
        format!("{:.1}B", n / ONE_BILLION)
    } else if n.abs() >= ONE_MILLION {
        format!("{:.1}M", n / ONE_MILLION)
    } else if n.abs() >= ONE_THOUSAND {
        format!("{:.1}K", n / ONE_THOUSAND)
    } else {
        format!("{:.0}", n)
    }
}
