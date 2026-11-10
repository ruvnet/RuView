//! .npz examples.

use ndarray::{array, Array2, ArrayRef2};
use ndarray_npy::{NpzReader, NpzWriter};
use std::{error::Error, io::Cursor};

#[test]
fn round_trip_npz() -> Result<(), Box<dyn Error>> {
    let mut buf = Vec::<u8>::new();

    let arr1 = array![[1i32, 3, 0], [4, 7, -1]];
    let arr2 = array![[9i32, 6], [-5, 2], [3, -1]];
    let arr3: &ArrayRef2<i32> = &arr1;

    {
        let mut writer = NpzWriter::new(Cursor::new(&mut buf));
        writer.add_array("arr1", &arr1)?;
        writer.add_array("arr2", &arr2)?;
        writer.add_array("arr3", arr3)?;
        writer.finish()?;
    }

    {
        let mut reader = NpzReader::new(Cursor::new(&buf))?;
        assert!(!reader.is_empty());
        assert_eq!(reader.len(), 3);
        assert_eq!(
            reader.names()?,
            vec!["arr1".to_string(), "arr2".to_string(), "arr3".to_string()],
        );
        {
            let by_name: Array2<i32> = reader.by_name("arr1")?;
            assert_eq!(by_name, arr1);
        }
        {
            let by_name: Array2<i32> = reader.by_name("arr1.npy")?;
            assert_eq!(by_name, arr1);
        }
        {
            let by_name: Array2<i32> = reader.by_name("arr2")?;
            assert_eq!(by_name, arr2);
        }
        {
            let by_name: Array2<i32> = reader.by_name("arr2.npy")?;
            assert_eq!(by_name, arr2);
        }
        {
            let by_name: Array2<i32> = reader.by_name("arr3")?;
            assert_eq!(*by_name, arr3);
        }
        {
            let by_name: Array2<i32> = reader.by_name("arr3.npy")?;
            assert_eq!(*by_name, arr3);
        }
        {
            let res: Result<Array2<i32>, _> = reader.by_name("arr1.npy.npy");
            assert!(res.is_err());
        }
    }

    Ok(())
}
