use pyo3::prelude::*;

#[pyfunction]
pub fn fee(amount: u64) -> u64 {
    if amount >= 100 {
        return 5;
    }
    0
}

#[pyclass]
pub struct Ledger {
    total: u64,
}

#[pymethods]
impl Ledger {
    pub fn charge(&mut self, amount: u64) -> u64 {
        if amount >= 100 {
            self.total += 5;
        }
        self.total
    }
}

#[pymodule]
fn feecalc(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(fee, m)?)?;
    Ok(())
}
