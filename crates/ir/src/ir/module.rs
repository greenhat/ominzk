use super::Func;
use super::FuncIndex;

pub struct Module {
    functions: Vec<Func>,
    pub start_func_idx: FuncIndex,
}

impl Module {
    pub fn new(functions: Vec<Func>, start_func_idx: FuncIndex) -> Self {
        Self {
            functions,
            start_func_idx,
        }
    }

    pub fn functions(&self) -> &[Func] {
        &self.functions
    }

    pub fn into_functions(self) -> Vec<Func> {
        self.functions
    }

    pub fn functions_mut(&mut self) -> &mut [Func] {
        &mut self.functions
    }

    pub fn function(&self, idx: u32) -> Option<&Func> {
        self.functions.get(idx as usize)
    }

    pub fn push_function(&mut self, func: Func) -> FuncIndex {
        self.functions.push(func);
        FuncIndex::from(self.functions.len() - 1)
    }

    pub fn set_function(&mut self, idx: u32, func: Func) {
        self.functions[idx as usize] = func;
    }
}
