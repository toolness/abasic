use core::fmt::Debug;
use std::{collections::HashMap, rc::Rc};

use crate::{
    interpreter_error::{InterpreterError, OutOfMemoryError, TracedInterpreterError},
    symbol::Symbol,
    value::Value,
};

/// The total number of elements can't exceed this amount,
/// or we'll feign an out of memory error.
const MAX_DIM_TOTAL_ELEMENTS: usize = 10000;

/// This is super weird and seems to be the default for Applesoft BASIC
/// and Commodore 64 BASIC.
const DEFAULT_ARRAY_SIZE: usize = 10;

#[derive(Default)]
pub struct Arrays(HashMap<Symbol, ValueArray>);

impl Debug for Arrays {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl Arrays {
    fn maybe_create_default_array(
        &mut self,
        array_name: &Symbol,
        dimensions: usize,
    ) -> Result<(), TracedInterpreterError> {
        // It seems we can't use hash_map::Entry here to provide a default value,
        // because we might actually error when creating the default value.
        if !self.0.contains_key(array_name) {
            let array = ValueArray::default_for_variable_and_dimensionality(
                &array_name.as_str(),
                dimensions,
            )?;
            self.0.insert(array_name.clone(), array);
        }
        Ok(())
    }

    pub fn create(
        &mut self,
        array_name: Symbol,
        max_indices: Vec<usize>,
    ) -> Result<(), TracedInterpreterError> {
        if self.has(&array_name) {
            return Err(InterpreterError::RedimensionedArray.into());
        }
        let array = ValueArray::create(array_name.as_str(), max_indices)?;
        self.0.insert(array_name, array);
        Ok(())
    }

    pub fn get_value_at_index(
        &mut self,
        array_name: &Symbol,
        index: &Vec<usize>,
    ) -> Result<Value, TracedInterpreterError> {
        self.maybe_create_default_array(array_name, index.len())?;
        let array = self.0.get(array_name).unwrap();

        Ok(array.get(index)?)
    }

    pub fn set_value_at_index(
        &mut self,
        array_name: &Symbol,
        index: &Vec<usize>,
        value: Value,
    ) -> Result<(), TracedInterpreterError> {
        value.validate_type_matches_variable_name(array_name.as_str())?;
        self.maybe_create_default_array(array_name, index.len())?;
        let array = self.0.get_mut(array_name).unwrap();
        array.set(index, value)?;
        Ok(())
    }

    pub fn has(&self, array_name: &Symbol) -> bool {
        self.0.contains_key(array_name)
    }
}

#[derive(Debug)]
pub enum ValueArray {
    String(DimArray<Rc<String>>),
    Number(DimArray<f64>),
}

impl ValueArray {
    pub fn default_for_variable_and_dimensionality(
        variable_name: &str,
        dimensions: usize,
    ) -> Result<Self, TracedInterpreterError> {
        let max_indices = vec![DEFAULT_ARRAY_SIZE; dimensions];
        Self::create(variable_name, max_indices)
    }

    pub fn create(
        variable_name: &str,
        max_indices: Vec<usize>,
    ) -> Result<Self, TracedInterpreterError> {
        if variable_name.ends_with('$') {
            Ok(ValueArray::String(DimArray::new(&max_indices)?))
        } else {
            Ok(ValueArray::Number(DimArray::new(&max_indices)?))
        }
    }

    pub fn set(&mut self, index: &[usize], value: Value) -> Result<(), TracedInterpreterError> {
        match self {
            ValueArray::String(ref mut array) => {
                array.set(index, value.try_into()?)?;
            }
            ValueArray::Number(array) => {
                array.set(index, value.try_into()?)?;
            }
        }
        Ok(())
    }

    pub fn get(&self, index: &[usize]) -> Result<Value, TracedInterpreterError> {
        match self {
            ValueArray::String(array) => Ok(array.get(index)?.into()),
            ValueArray::Number(array) => Ok(array.get(index)?.into()),
        }
    }
}

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

    pub fn get(&self, index: &[usize]) -> Result<T, InterpreterError> {
        let linear_index = self.get_linear_index(index)?;
        Ok(self.values[linear_index].clone())
    }

    pub fn set(&mut self, index: &[usize], value: T) -> Result<(), InterpreterError> {
        let linear_index = self.get_linear_index(index)?;
        self.values[linear_index] = value;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::interpreter_error::{InterpreterError, OutOfMemoryError};

    use super::DimArray;

    #[test]
    fn zero_dimensional_arrays_return_err() {
        assert_eq!(
            DimArray::<u8>::new(&[]),
            Err(InterpreterError::BadSubscript)
        );
    }

    #[test]
    fn too_large_arrays_return_err() {
        assert_eq!(
            DimArray::<u8>::new(&[99999, 99999]),
            Err(OutOfMemoryError::ArrayTooLarge.into())
        );
    }

    #[test]
    fn single_element_arrays_work() {
        let mut arr = DimArray::<u8>::new(&[0]).unwrap();
        assert_eq!(arr.get(&[0]).unwrap(), 0);
        arr.set(&[0], 15).unwrap();
        assert_eq!(arr.get(&[0]).unwrap(), 15);
        assert_eq!(arr.get(&[1]), Err(InterpreterError::BadSubscript));
        assert_eq!(arr.get(&[]), Err(InterpreterError::BadSubscript));
        assert_eq!(arr.set(&[], 1), Err(InterpreterError::BadSubscript));
        assert_eq!(arr.set(&[1], 1), Err(InterpreterError::BadSubscript));
    }

    #[test]
    fn one_dimensional_arrays_work() {
        let mut arr = DimArray::<u8>::new(&[1]).unwrap();
        arr.set(&[1], 20).unwrap();
        arr.set(&[0], 15).unwrap();
        assert_eq!(arr.get(&[1]).unwrap(), 20);
        assert_eq!(arr.get(&[0]).unwrap(), 15);
        assert_eq!(arr.get(&[2]), Err(InterpreterError::BadSubscript));
        assert_eq!(arr.get(&[]), Err(InterpreterError::BadSubscript));
        assert_eq!(arr.set(&[], 1), Err(InterpreterError::BadSubscript));
        assert_eq!(arr.set(&[2], 1), Err(InterpreterError::BadSubscript));
    }

    #[test]
    fn two_dimensional_arrays_work() {
        let mut arr = DimArray::<u8>::new(&[1, 1]).unwrap();
        arr.set(&[0, 0], 1).unwrap();
        arr.set(&[0, 1], 2).unwrap();
        arr.set(&[1, 0], 3).unwrap();
        arr.set(&[1, 1], 4).unwrap();
        assert_eq!(arr.get(&[0, 0]).unwrap(), 1);
        assert_eq!(arr.get(&[0, 1]).unwrap(), 2);
        assert_eq!(arr.get(&[1, 0]).unwrap(), 3);
        assert_eq!(arr.get(&[1, 1]).unwrap(), 4);
        assert_eq!(arr.get(&[0]), Err(InterpreterError::BadSubscript));
        assert_eq!(arr.get(&[0, 2]), Err(InterpreterError::BadSubscript));
    }

    #[test]
    fn three_dimensional_arrays_work() {
        let mut arr = DimArray::<u8>::new(&[1, 1, 1]).unwrap();
        arr.set(&[0, 0, 0], 1).unwrap();
        arr.set(&[0, 1, 0], 2).unwrap();
        arr.set(&[1, 0, 0], 3).unwrap();
        arr.set(&[1, 1, 0], 4).unwrap();
        arr.set(&[0, 0, 1], 5).unwrap();
        arr.set(&[0, 1, 1], 6).unwrap();
        arr.set(&[1, 0, 1], 7).unwrap();
        arr.set(&[1, 1, 1], 8).unwrap();
        assert_eq!(arr.get(&[0, 0, 0]).unwrap(), 1);
        assert_eq!(arr.get(&[0, 1, 0]).unwrap(), 2);
        assert_eq!(arr.get(&[1, 0, 0]).unwrap(), 3);
        assert_eq!(arr.get(&[1, 1, 0]).unwrap(), 4);
        assert_eq!(arr.get(&[0, 0, 1]).unwrap(), 5);
        assert_eq!(arr.get(&[0, 1, 1]).unwrap(), 6);
        assert_eq!(arr.get(&[1, 0, 1]).unwrap(), 7);
        assert_eq!(arr.get(&[1, 1, 1]).unwrap(), 8);
        assert_eq!(arr.get(&[0]), Err(InterpreterError::BadSubscript));
        assert_eq!(arr.get(&[0, 0]), Err(InterpreterError::BadSubscript));
        assert_eq!(arr.get(&[0, 0, 2]), Err(InterpreterError::BadSubscript));
    }
}
