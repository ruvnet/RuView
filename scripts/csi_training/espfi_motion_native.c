/* Fixed-shape centered ACF reductions. No input mutation, allocation or state. */
#include <stddef.h>
#include <math.h>

unsigned int espfi_motion_abi(void) { return 1u; }

int espfi_motion_features(const double *x, size_t input_count,
                          double *out, size_t output_count) {
    static const size_t acf_lags[6] = {1,2,4,8,16,32};
    static const size_t diff_lags[3] = {1,4,16};
    double energy[52];
    double total = 0.0;
    if (!x || !out || input_count != 9880 || output_count != 468) return 1;
    for (size_t c=0;c<52;c++) {
        double sum=0.0;
        for (size_t t=0;t<190;t++) {
            double v=x[c*190+t];
            if (!isfinite(v)) return 2;
            sum += v*v;
        }
        if (!isfinite(sum)) return 2;
        energy[c]=sum;
        total += sum;
    }
    double scale=sqrt(total/9880.0);
    if (!isfinite(scale)) return 2;
    if (scale<1e-6) scale=1e-6;
    for (size_t c=0;c<52;c++) {
        const double *row=x+c*190;
        double denominator=energy[c]>190e-12?energy[c]:190e-12;
        for (size_t k=0;k<6;k++) {
            size_t lag=acf_lags[k];
            double sum=0.0;
            for (size_t t=lag;t<190;t++) sum += row[t]*row[t-lag];
            out[c*9+k]=sum/denominator;
        }
        for (size_t k=0;k<3;k++) {
            size_t lag=diff_lags[k];
            double sum=0.0;
            for (size_t t=lag;t<190;t++) sum += fabs(row[t]-row[t-lag]);
            out[c*9+6+k]=(sum/(double)(190-lag))/scale;
        }
    }
    for (size_t i=0;i<468;i++) if (!isfinite(out[i])) return 2;
    return 0;
}
