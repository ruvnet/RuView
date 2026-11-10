"""This script generates initial files for the tests in `examples.rs`.

Afterwards, the files will be manually modified to account for slight
differences in the writer implementations. (For example, NumPy tends to write a
trailing comma in the Python dictionary literal that needs to be removed to
match the output of `py_literal`.)

"""


import numpy as np


def write_example_array(shape, dtype, order, f, dst):
    arr = np.zeros(shape, dtype=dtype, order=order)
    for i in range(arr.size):
        arr.flat[i] = f(i)
    np.save(dst, arr)


def main():
    write_example_array(
        (2, 3), dtype='<i4', order='C', f=lambda i: i,
        dst='array.npy',
    )

    SHAPE = (2, 3, 4)

    write_example_array(
        SHAPE, dtype='<f8', order='C', f=lambda i: i,
        dst='example_f64_little_endian_standard.npy',
    )
    write_example_array(
        SHAPE, dtype='<f8', order='F', f=lambda i: i,
        dst='example_f64_little_endian_fortran.npy',
    )
    write_example_array(
        SHAPE, dtype='>f8', order='C', f=lambda i: i,
        dst='example_f64_big_endian_standard.npy',
    )
    write_example_array(
        SHAPE, dtype='>f8', order='F', f=lambda i: i,
        dst='example_f64_big_endian_fortran.npy',
    )
    write_example_array(
        SHAPE, dtype='<c16', order='C', f=lambda i: i - i * 1j,
        dst='example_c64_little_endian_standard.npy',
    )
    write_example_array(
        SHAPE, dtype='<c16', order='F', f=lambda i: i - i * 1j,
        dst='example_c64_little_endian_fortran.npy',
    )
    write_example_array(
        SHAPE, dtype='>c16', order='C', f=lambda i: i - i * 1j,
        dst='example_c64_big_endian_standard.npy',
    )
    write_example_array(
        SHAPE, dtype='>c16', order='F', f=lambda i: i - i * 1j,
        dst='example_c64_big_endian_fortran.npy',
    )
    write_example_array(
        SHAPE, dtype='?', order='C', f=lambda i: (i % 5) % 2 == 0,
        dst='example_bool_standard.npy',
    )


if __name__ == '__main__':
    main()
