use crate::interpreter_error::{InterpreterError, OutOfMemoryError};

/// The total number of elements can't exceed this amount,
/// or we'll feign an out of memory error.
const MAX_DIM_TOTAL_ELEMENTS: usize = 10000;

#[derive(Debug, PartialEq)]
pub struct DimArray<T: Default + Clone> {
    values: Vec<T>,
    dimensions: Vec<usize>,
}

impl<T: Default + Clone> DimArray<T> {
    pub fn new(max_indices: &[usize]) -> Result<Self, InterpreterError> {
        if max_indices.len() == 0 {
            // Technically Applesoft BASIC returns a syntax error for this
            // but bad subscript seems more specific.
            return Err(InterpreterError::BadSubscript);
        }
        let mut dimensions = Vec::with_capacity(max_indices.len());
        let mut total_elements = 1;
        for &max_index in max_indices {
            // DIM declarations in BASIC represent the maximum index along each axis,
            // not the size along each axis, so we have to increment the number by 1.
            let dimension_size = max_index + 1;
            total_elements *= dimension_size;
            dimensions.push(dimension_size);
        }
        if total_elements > MAX_DIM_TOTAL_ELEMENTS {
            return Err(OutOfMemoryError::ArrayTooLarge.into());
        }
        let values = vec![T::default(); total_elements];
        Ok(DimArray { values, dimensions })
    }

    fn get_linear_index(&self, indices: &[usize]) -> Result<usize, InterpreterError> {
        if indices.len() != self.dimensions.len() {
            return Err(InterpreterError::BadSubscript);
        }
        let mut linear_index = 0;
        let mut stride: usize = 1;
        for (&dim_index, &dim_size) in std::iter::zip(indices, &self.dimensions) {
            if dim_index >= dim_size {
                return Err(InterpreterError::BadSubscript);
            }
            linear_index += dim_index * stride;
            stride *= dim_size;
        }
        Ok(linear_index)
    }

    pub fn get(&self, index: &[usize]) -> Result<&T, InterpreterError> {
        let linear_index = self.get_linear_index(index)?;
        Ok(&self.values[linear_index])
    }

    pub fn set(&mut self, index: &[usize], value: T) -> Result<(), InterpreterError> {
        let linear_index = self.get_linear_index(index)?;
        self.values[linear_index] = value;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::interpreter_error::InterpreterError;

    use super::DimArray;

    #[test]
    fn zero_dimensional_arrays_return_err() {
        assert_eq!(
            DimArray::<u8>::new(&[]),
            Err(InterpreterError::BadSubscript)
        );
    }

    #[test]
    fn single_element_arrays_work() {
        let mut arr = DimArray::<u8>::new(&[0]).unwrap();
        assert_eq!(*arr.get(&[0]).unwrap(), 0);
        arr.set(&[0], 15).unwrap();
        assert_eq!(*arr.get(&[0]).unwrap(), 15);
        assert_eq!(arr.get(&[1]), Err(InterpreterError::BadSubscript));
    }
}
