#[derive(Debug, Clone)]
pub enum Documento {
    Texto(Vec<String>),
    Calculo(Vec<Vec<String>>), // Matriz para hoja de cálculo
}

impl Default for Documento {
    fn default() -> Self {
        Documento::Texto(Vec::new())
    }
}

impl Documento {
    pub fn as_texto_mut(&mut self) -> Option<&mut Vec<String>> {
        match self {
            Documento::Texto(ref mut v) => Some(v),
            _ => None,
        }
    }
    pub fn as_texto(&self) -> Option<&Vec<String>> {
        match self {
            Documento::Texto(ref v) => Some(v),
            _ => None,
        }
    }
    pub fn len(&self) -> usize {
        match self {
            Documento::Texto(v) => v.len(),
            Documento::Calculo(m) => m.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn join(&self, sep: &str) -> Option<String> {
        match self {
            Documento::Texto(v) => Some(v.join(sep)),
            _ => None,
        }
    }
    pub fn iter(&self) -> Option<std::slice::Iter<'_, String>> {
        match self {
            Documento::Texto(v) => Some(v.iter()),
            _ => None,
        }
    }
    pub fn insert(&mut self, idx: usize, val: String) {
        if let Some(v) = self.as_texto_mut() {
            v.insert(idx, val);
        }
    }
    pub fn push(&mut self, val: String) {
        if let Some(v) = self.as_texto_mut() {
            v.push(val);
        }
    }
    pub fn get_mut(&mut self, idx: usize) -> Option<&mut String> {
        if let Some(v) = self.as_texto_mut() {
            v.get_mut(idx)
        } else {
            None
        }
    }
}
