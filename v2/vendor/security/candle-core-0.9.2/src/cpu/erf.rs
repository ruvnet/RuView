#![allow(clippy::excessive_precision)]
// Code taken from https://github.com/statrs-dev/statrs
//! Provides the [error](https://en.wikipedia.org/wiki/Error_function) and
//! related functions

mod evaluate {
    //! Provides functions that don't have a numerical solution and must
    //! be solved computationally (e.g. evaluation of a polynomial)

    /// evaluates a polynomial at `z` where `coeff` are the coefficients
    /// to a polynomial of order `k` where `k` is the length of `coeff` and the
    /// coeffecient
    /// to the `k`th power is the `k`th element in coeff. E.g. [3,-1,2] equates to
    /// `2z^2 - z + 3`
    ///
    /// # Remarks
    ///
    /// Returns 0 for a 0 length coefficient slice
    pub fn polynomial(z: f64, coeff: &[f64]) -> f64 {
        let n = coeff.len();
        if n == 0 {
            return 0.0;
        }

        let mut sum = *coeff.last().unwrap();
        for c in coeff[0..n - 1].iter().rev() {
            sum = *c + z * sum;
        }
        sum
    }
}
use std::f64;

/// `erf` calculates the error function at `x`.
pub fn erf_f64(x: f64) -> f64 {
    libm::erf(x)
}

pub fn erf_f32(x: f32) -> f32 {
    libm::erff(x)
}

/// `erf_inv` calculates the inverse error function
/// at `x`.
pub fn erf_inv(x: f64) -> f64 {
    if x == 0.0 {
        0.0
    } else if x >= 1.0 {
        f64::INFINITY
    } else if x <= -1.0 {
        f64::NEG_INFINITY
    } else if x < 0.0 {
        erf_inv_impl(-x, 1.0 + x, -1.0)
    } else {
        erf_inv_impl(x, 1.0 - x, 1.0)
    }
}

/// `erfc` calculates the complementary error function
/// at `x`.
pub fn erfc_f64(x: f64) -> f64 {
    libm::erfc(x)
}

pub fn erfc_f32(x: f32) -> f32 {
    libm::erfcf(x)
}

/// `erfc_inv` calculates the complementary inverse
/// error function at `x`.
pub fn erfc_inv(x: f64) -> f64 {
    if x <= 0.0 {
        f64::INFINITY
    } else if x >= 2.0 {
        f64::NEG_INFINITY
    } else if x > 1.0 {
        erf_inv_impl(-1.0 + x, 2.0 - x, -1.0)
    } else {
        erf_inv_impl(1.0 - x, x, 1.0)
    }
}

// **********************************************************
// ********** Coefficients for erf_inv_impl polynomial ******
// **********************************************************

/// Polynomial coefficients for a numerator of `erf_inv_impl`
/// in the interval [0, 0.5].
const ERF_INV_IMPL_AN: &[f64] = &[
    -0.000508781949658280665617,
    -0.00836874819741736770379,
    0.0334806625409744615033,
    -0.0126926147662974029034,
    -0.0365637971411762664006,
    0.0219878681111168899165,
    0.00822687874676915743155,
    -0.00538772965071242932965,
];

/// Polynomial coefficients for a denominator of `erf_inv_impl`
/// in the interval [0, 0.5].
const ERF_INV_IMPL_AD: &[f64] = &[
    1.0,
    -0.970005043303290640362,
    -1.56574558234175846809,
    1.56221558398423026363,
    0.662328840472002992063,
    -0.71228902341542847553,
    -0.0527396382340099713954,
    0.0795283687341571680018,
    -0.00233393759374190016776,
    0.000886216390456424707504,
];

/// Polynomial coefficients for a numerator of `erf_inv_impl`
/// in the interval [0.5, 0.75].
const ERF_INV_IMPL_BN: &[f64] = &[
    -0.202433508355938759655,
    0.105264680699391713268,
    8.37050328343119927838,
    17.6447298408374015486,
    -18.8510648058714251895,
    -44.6382324441786960818,
    17.445385985570866523,
    21.1294655448340526258,
    -3.67192254707729348546,
];

/// Polynomial coefficients for a denominator of `erf_inv_impl`
/// in the interval [0.5, 0.75].
const ERF_INV_IMPL_BD: &[f64] = &[
    1.0,
    6.24264124854247537712,
    3.9713437953343869095,
    -28.6608180499800029974,
    -20.1432634680485188801,
    48.5609213108739935468,
    10.8268667355460159008,
    -22.6436933413139721736,
    1.72114765761200282724,
];

/// Polynomial coefficients for a numerator of `erf_inv_impl`
/// in the interval [0.75, 1] with x less than 3.
const ERF_INV_IMPL_CN: &[f64] = &[
    -0.131102781679951906451,
    -0.163794047193317060787,
    0.117030156341995252019,
    0.387079738972604337464,
    0.337785538912035898924,
    0.142869534408157156766,
    0.0290157910005329060432,
    0.00214558995388805277169,
    -0.679465575181126350155e-6,
    0.285225331782217055858e-7,
    -0.681149956853776992068e-9,
];

/// Polynomial coefficients for a denominator of `erf_inv_impl`
/// in the interval [0.75, 1] with x less than 3.
const ERF_INV_IMPL_CD: &[f64] = &[
    1.0,
    3.46625407242567245975,
    5.38168345707006855425,
    4.77846592945843778382,
    2.59301921623620271374,
    0.848854343457902036425,
    0.152264338295331783612,
    0.01105924229346489121,
];

/// Polynomial coefficients for a numerator of `erf_inv_impl`
/// in the interval [0.75, 1] with x between 3 and 6.
const ERF_INV_IMPL_DN: &[f64] = &[
    -0.0350353787183177984712,
    -0.00222426529213447927281,
    0.0185573306514231072324,
    0.00950804701325919603619,
    0.00187123492819559223345,
    0.000157544617424960554631,
    0.460469890584317994083e-5,
    -0.230404776911882601748e-9,
    0.266339227425782031962e-11,
];

/// Polynomial coefficients for a denominator of `erf_inv_impl`
/// in the interval [0.75, 1] with x between 3 and 6.
const ERF_INV_IMPL_DD: &[f64] = &[
    1.0,
    1.3653349817554063097,
    0.762059164553623404043,
    0.220091105764131249824,
    0.0341589143670947727934,
    0.00263861676657015992959,
    0.764675292302794483503e-4,
];

/// Polynomial coefficients for a numerator of `erf_inv_impl`
/// in the interval [0.75, 1] with x between 6 and 18.
const ERF_INV_IMPL_EN: &[f64] = &[
    -0.0167431005076633737133,
    -0.00112951438745580278863,
    0.00105628862152492910091,
    0.000209386317487588078668,
    0.149624783758342370182e-4,
    0.449696789927706453732e-6,
    0.462596163522878599135e-8,
    -0.281128735628831791805e-13,
    0.99055709973310326855e-16,
];

/// Polynomial coefficients for a denominator of `erf_inv_impl`
/// in the interval [0.75, 1] with x between 6 and 18.
const ERF_INV_IMPL_ED: &[f64] = &[
    1.0,
    0.591429344886417493481,
    0.138151865749083321638,
    0.0160746087093676504695,
    0.000964011807005165528527,
    0.275335474764726041141e-4,
    0.282243172016108031869e-6,
];

/// Polynomial coefficients for a numerator of `erf_inv_impl`
/// in the interval [0.75, 1] with x between 18 and 44.
const ERF_INV_IMPL_FN: &[f64] = &[
    -0.0024978212791898131227,
    -0.779190719229053954292e-5,
    0.254723037413027451751e-4,
    0.162397777342510920873e-5,
    0.396341011304801168516e-7,
    0.411632831190944208473e-9,
    0.145596286718675035587e-11,
    -0.116765012397184275695e-17,
];

/// Polynomial coefficients for a denominator of `erf_inv_impl`
/// in the interval [0.75, 1] with x between 18 and 44.
const ERF_INV_IMPL_FD: &[f64] = &[
    1.0,
    0.207123112214422517181,
    0.0169410838120975906478,
    0.000690538265622684595676,
    0.145007359818232637924e-4,
    0.144437756628144157666e-6,
    0.509761276599778486139e-9,
];

/// Polynomial coefficients for a numerator of `erf_inv_impl`
/// in the interval [0.75, 1] with x greater than 44.
const ERF_INV_IMPL_GN: &[f64] = &[
    -0.000539042911019078575891,
    -0.28398759004727721098e-6,
    0.899465114892291446442e-6,
    0.229345859265920864296e-7,
    0.225561444863500149219e-9,
    0.947846627503022684216e-12,
    0.135880130108924861008e-14,
    -0.348890393399948882918e-21,
];

/// Polynomial coefficients for a denominator of `erf_inv_impl`
/// in the interval [0.75, 1] with x greater than 44.
const ERF_INV_IMPL_GD: &[f64] = &[
    1.0,
    0.0845746234001899436914,
    0.00282092984726264681981,
    0.468292921940894236786e-4,
    0.399968812193862100054e-6,
    0.161809290887904476097e-8,
    0.231558608310259605225e-11,
];

// `erf_inv_impl` computes the inverse error function where
// `p`,`q`, and `s` are the first, second, and third intermediate
// parameters respectively
fn erf_inv_impl(p: f64, q: f64, s: f64) -> f64 {
    let result = if p <= 0.5 {
        let y = 0.0891314744949340820313;
        let g = p * (p + 10.0);
        let r = evaluate::polynomial(p, ERF_INV_IMPL_AN) / evaluate::polynomial(p, ERF_INV_IMPL_AD);
        g * y + g * r
    } else if q >= 0.25 {
        let y = 2.249481201171875;
        let g = (-2.0 * q.ln()).sqrt();
        let xs = q - 0.25;
        let r =
            evaluate::polynomial(xs, ERF_INV_IMPL_BN) / evaluate::polynomial(xs, ERF_INV_IMPL_BD);
        g / (y + r)
    } else {
        let x = (-q.ln()).sqrt();
        if x < 3.0 {
            let y = 0.807220458984375;
            let xs = x - 1.125;
            let r = evaluate::polynomial(xs, ERF_INV_IMPL_CN)
                / evaluate::polynomial(xs, ERF_INV_IMPL_CD);
            y * x + r * x
        } else if x < 6.0 {
            let y = 0.93995571136474609375;
            let xs = x - 3.0;
            let r = evaluate::polynomial(xs, ERF_INV_IMPL_DN)
                / evaluate::polynomial(xs, ERF_INV_IMPL_DD);
            y * x + r * x
        } else if x < 18.0 {
            let y = 0.98362827301025390625;
            let xs = x - 6.0;
            let r = evaluate::polynomial(xs, ERF_INV_IMPL_EN)
                / evaluate::polynomial(xs, ERF_INV_IMPL_ED);
            y * x + r * x
        } else if x < 44.0 {
            let y = 0.99714565277099609375;
            let xs = x - 18.0;
            let r = evaluate::polynomial(xs, ERF_INV_IMPL_FN)
                / evaluate::polynomial(xs, ERF_INV_IMPL_FD);
            y * x + r * x
        } else {
            let y = 0.99941349029541015625;
            let xs = x - 44.0;
            let r = evaluate::polynomial(xs, ERF_INV_IMPL_GN)
                / evaluate::polynomial(xs, ERF_INV_IMPL_GD);
            y * x + r * x
        }
    };
    s * result
}
