macro_rules! define_tree_with_extensions {
    (
        $base_name:ident,
        {
            $($ext_field:ident: $ext_type:ty),* $(,)?
        }
    ) => {
        // Define the extended struct
        #[derive(Debug)]
        struct $base_name {
            // Original fields will be in the inner struct
            inner: $base_name, // This will be the original struct
            $($ext_field: $ext_type,)*
        }

        // Implement Deref/DerefMut to access original fields transparently
        impl std::ops::Deref for $base_name {
            type Target = $base_name; // Original struct

            fn deref(&self) -> &Self::Target {
                &self.inner
            }
        }

        impl std::ops::DerefMut for $base_name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.inner
            }
        }

        // Constructor
        impl $base_name {
            fn new(inner: $base_name, $($ext_field: $ext_type),*) -> Self {
                Self {
                    inner,
                    $($ext_field,)*
                }
            }

            // Direct access to extended fields
            $(
                pub fn $ext_field(&self) -> &$ext_type {
                    &self.$ext_field
                }

                pub fn $ext_field _mut(&mut self) -> &mut $ext_type {
                    &mut self.$ext_field
                }
            )*
        }
    };

    // Special case: if the original struct doesn't exist, define both
    (
        $base_name:ident {
            $($base_field:ident: $base_type:ty),* $(,)?
        },
        $extended_name:ident {
            $($ext_field:ident: $ext_type:ty),* $(,)?
        }
    ) => {
        // Original struct
        #[derive(Debug)]
        struct $base_name {
            $($base_field: $base_type,)*
            parent: ParentRef<$base_name>,
            sub_dirs: Vec<Rc<RefCell<$base_name>>>,
            sub_files: Vec<Rc<RefCell<$base_name>>>,
        }

        // Extended struct with transparent access
        #[derive(Debug)]
        struct $extended_name {
            inner: $base_name,
            $($ext_field: $ext_type,)*
        }

        // Deref implementation for transparent access
        impl std::ops::Deref for $extended_name {
            type Target = $base_name;

            fn deref(&self) -> &Self::Target {
                &self.inner
            }
        }

        impl std::ops::DerefMut for $extended_name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.inner
            }
        }

        // Constructor and access methods
        impl $extended_name {
            fn new(inner: $base_name, $($ext_field: $ext_type),*) -> Self {
                Self {
                    inner,
                    $($ext_field,)*
                }
            }

            // Direct access to extended fields
            $(
                pub fn $ext_field(&self) -> &$ext_type {
                    &self.$ext_field
                }

                pub fn $ext_field _mut(&mut self) -> &mut $ext_type {
                    &mut self.$ext_field
                }
            )*
        }

        // Conversion functions
        impl $extended_name {
            fn from_base(base: $base_name, $($ext_field: $ext_type),*) -> Self {
                Self::new(base, $($ext_field),*)
            }

            fn into_base(self) -> $base_name {
                self.inner
            }

            fn as_base(&self) -> &$base_name {
                &self.inner
            }

            fn as_base_mut(&mut self) -> &mut $base_name {
                &mut self.inner
            }
        }
    };
}

// For your specific case, you probably want this version that generates Extended_ prefix:
macro_rules! extend_struct {
    (
        $base_name:ident {
            $($base_field:ident: $base_type:ty),* $(,)?
        },
        {
            $($ext_field:ident: $ext_type:ty),* $(,)?
        }
    ) => {
        // Original struct (if it doesn't exist)
        #[derive(Debug)]
        struct $base_name {
            $($base_field: $base_type,)*
            parent: ParentRef<$base_name>,
            sub_dirs: Vec<Rc<RefCell<$base_name>>>,
            sub_files: Vec<Rc<RefCell<$base_name>>>,
        }

        // Extended struct with Extended_ prefix
        paste::paste! {
            #[derive(Debug)]
            struct [<$base_name Extended>] {
                inner: $base_name,
                $($ext_field: $ext_type,)*
            }

            // Deref for transparent access
            impl std::ops::Deref for [<$base_name Extended>] {
                type Target = $base_name;

                fn deref(&self) -> &Self::Target {
                    &self.inner
                }
            }

            impl std::ops::DerefMut for [<$base_name Extended>] {
                fn deref_mut(&mut self) -> &mut Self::Target {
                    &mut self.inner
                }
            }

            // Constructor and access methods
            impl [<$base_name Extended>] {
                fn new(inner: $base_name, $($ext_field: $ext_type),*) -> Self {
                    Self {
                        inner,
                        $($ext_field,)*
                    }
                }

                // Direct access to extended fields
                $(
                    pub fn $ext_field(&self) -> &$ext_type {
                        &self.$ext_field
                    }

                    pub fn $ext_field _mut(&mut self) -> &mut $ext_type {
                        &mut self.$ext_field
                    }
                )*

                // Conversion methods
                pub fn from_base(base: $base_name, $($ext_field: $ext_type),*) -> Self {
                    Self::new(base, $($ext_field),*)
                }

                pub fn into_base(self) -> $base_name {
                    self.inner
                }

                pub fn as_base(&self) -> &$base_name {
                    &self.inner
                }

                pub fn as_base_mut(&mut self) -> &mut $base_name {
                    &mut self.inner
                }
            }
        }
    };
}

/*
// Conversion functions for Rc<RefCell<>> trees
impl DirInfoExtended {
    fn convert_tree(
        original: &Rc<RefCell<DirInfo>>,
        make_ext_data: impl Fn(&DirInfo) -> (Vec<bool>, Vec<bool>) // your extension data
    ) -> Rc<RefCell<DirInfoExtended>> {
        let original_borrow = original.borrow();

        let (match_details, display_data) = make_ext_data(&*original_borrow);

        let extended = Rc::new(RefCell::new(DirInfoExtended::from_base(
            DirInfo {
                path: original_borrow.path.clone(),
                name: original_borrow.name.clone(),
                immediate_files_size: original_borrow.immediate_files_size,
                total_size: original_borrow.total_size,
                regex_matched: original_borrow.regex_matched,
                contains_dir_matching_regex: original_borrow.contains_dir_matching_regex,
                contains_file_matching_regex: original_borrow.contains_file_matching_regex,
                depth: original_borrow.depth,
                parent: ParentRef::none(), // Will fix up later
                metadata: original_borrow.metadata.clone(),
                sub_dirs: Vec::new(),
                sub_files: Vec::new(),
            },
            match_details,
            display_data
        )));

        // Convert children
        let mut extended_borrow = extended.borrow_mut();

        // Convert files
        for original_file in &original_borrow.sub_files {
            let file_borrow = original_file.borrow();
            // Convert file to extended version (you'll need a similar function for files)
            // extended_borrow.sub_files.push(convert_file(...));
        }

        // Convert directories
        for original_dir in &original_borrow.sub_dirs {
            let extended_child = Self::convert_tree(original_dir, &make_ext_data);
            extended_borrow.sub_dirs.push(extended_child.clone());
        }

        drop(extended_borrow);

        // Fix up parent references
        {
            let extended_ref = extended.borrow();
            for child_dir in &extended_ref.sub_dirs {
                child_dir.borrow_mut().parent = ParentRef::from_rc(&extended);
            }
        }

        extended
    }
}
*/

// Usage example:
// extend_struct!(
//     DirInfo {
//         path: PathBuf,
//         name: String,
//         total_size: u64,
//     },
//     {
//         match_details: Vec<bool>,
//         display_data: Vec<bool>,
//     }
// );
//
// let dir = DirInfo {
//     path: PathBuf::new(),
//     name: "test".to_string(),
//     total_size: 100,
//     parent: ParentRef::none(),
//     sub_dirs: Vec::new(),
//     sub_files: Vec::new(),
// };
//
// let extended_dir = DirInfoExtended::from_base(
//     dir,
//     vec![true, false],  // match_details
//     vec![true, true]    // display_data
// );
//
// // Now you can access both original and extended fields:
// println!("{}", extended_dir.name);           // Original field (via Deref)
// println!("{:?}", extended_dir.match_details()); // Extended field