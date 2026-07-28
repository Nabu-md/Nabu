#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdfkit_load() {
        // Just try to see if we can declare a PDFDocument class
        unsafe {
             let cls = objc2::class!(PDFDocument);
             assert!(!cls.is_null());
        }
    }
}
