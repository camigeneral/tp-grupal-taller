#[allow(dead_code)]
pub fn lset(list: &mut [String], index: i32, element: &str) {
    let index_usize = if index < 0 {
        let abs_index = index.unsigned_abs() as usize;
        list.len() - abs_index
    } else {
        index as usize
    };
    list[index_usize] = element.to_string();
}

#[allow(dead_code)]
pub fn llen(list: &[String]) -> usize {
    list.len()
}

#[allow(dead_code)]
pub fn rpush(list: &mut Vec<String>, values: Vec<String>) {
    for value in values {
        list.push(value);
    }
}

#[allow(dead_code)]
pub fn linsert(list: &mut Vec<String>, flag: String, pivot: String, element: String) {
    if let Some(index) = list.iter().position(|x| *x == pivot) {
        match flag.to_lowercase().as_str() {
            "before" => {
                list.insert(index, element);
            }
            "after" => {
                if list.len() > index + 1 {
                    list.insert(index + 1, element);
                } else {
                    list.push(element);
                }
            }
            _ => {}
        }
    }
}

//------------------------------------------------------------------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lset() {
        let mut list: Vec<String> = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        lset(&mut list, 0, "4");
        lset(&mut list, -2, "5");
        assert_eq!(list[0], "4");
        assert_eq!(list[1], "5");
        assert_eq!(list[2], "3");
    }

    #[test]
    fn test_llen() {
        let list: Vec<String> = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        assert_eq!(llen(&list), 3);
    }

    #[test]
    fn test_rpush() {
        let mut list: Vec<String> = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        let values: Vec<String> = vec!["4".to_string(), "5".to_string()];
        rpush(&mut list, values);
        assert_eq!(list[0], "1");
        assert_eq!(list[1], "2");
        assert_eq!(list[2], "3");
        assert_eq!(list[3], "4");
        assert_eq!(list[4], "5");
    }

    #[test]
    fn test_linsert() {
        let mut list: Vec<String> = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        linsert(
            &mut list,
            "before".to_string(),
            "2".to_string(),
            "4".to_string(),
        );
        assert_eq!(list[0], "1");
        assert_eq!(list[1], "4");
        assert_eq!(list[2], "2");
        assert_eq!(list[3], "3");

        linsert(
            &mut list,
            "after".to_string(),
            "2".to_string(),
            "5".to_string(),
        );
        assert_eq!(list[0], "1");
        assert_eq!(list[1], "4");
        assert_eq!(list[2], "2");
        assert_eq!(list[3], "5");
        assert_eq!(list[4], "3");
    }
}
