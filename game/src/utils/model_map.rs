use fyrox::{engine::resource_manager::ResourceManager, resource::model::Model};
use std::{collections::HashMap, ops::Index, path::Path};

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
            map: fyrox::core::futures::future::join_all(
                paths
                    .into_iter()
                    .map(|path| resource_manager.request_model(path))
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
