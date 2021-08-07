use rg3d::{
    engine::resource_manager::{MaterialSearchOptions, ResourceManager},
    resource::model::Model,
};
use std::{
    collections::HashMap,
    ops::Index,
    path::{Path, PathBuf},
};

pub struct ModelMap {
    pub map: HashMap<String, Model>,
}

impl ModelMap {
    pub async fn new<I>(paths: I, resource_manager: ResourceManager) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<Path>,
    {
        Self {
            map: rg3d::core::futures::future::join_all(
                paths
                    .into_iter()
                    .map(|path| {
                        resource_manager.request_model(
                            path,
                            MaterialSearchOptions::MaterialsDirectory(PathBuf::from(
                                "data/textures",
                            )),
                        )
                    })
                    .collect::<Vec<_>>(),
            )
            .await
            .into_iter()
            .map(|r| {
                let resource = r.unwrap();
                let key = resource.state().path().to_string_lossy().into_owned();
                (key, resource)
            })
            .collect::<HashMap<_, _>>(),
        }
    }
}

impl<T: AsRef<str>> Index<T> for ModelMap {
    type Output = Model;

    fn index(&self, index: T) -> &Self::Output {
        self.map.get(index.as_ref()).unwrap()
    }
}
