pub fn mse(y: &[f32], yhat: &[f32]) -> f32 {
    y.iter().zip(yhat).map(|(a,b)| (a-b)*(a-b)).sum::<f32>() / y.len() as f32
}

pub fn acc(y: &[usize], yhat: &[usize]) -> f32 {
    let c = y.iter().zip(yhat).filter(|(a,b)| a==b).count();
    c as f32 / y.len() as f32
}