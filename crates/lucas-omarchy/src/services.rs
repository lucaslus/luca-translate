use crate::config;
use lucas_core::services::{
    bing_free::BingFree, deepl_free::DeepLFree, google_free::GoogleFree,
    openai_compat::OpenAiCompat, youdao_dict::YoudaoDict, TranslateService,
};

pub fn build_services(
    ai: &config::AiConfig,
    toggles: &config::ServiceToggles,
) -> Vec<Box<dyn TranslateService>> {
    let mut services: Vec<Box<dyn TranslateService>> = Vec::new();
    if toggles.youdao {
        services.push(Box::new(YoudaoDict));
    }
    if toggles.deepl {
        services.push(Box::new(DeepLFree::new()));
    }
    if toggles.bing {
        services.push(Box::new(BingFree));
    }
    if toggles.google {
        services.push(Box::new(GoogleFree));
    }
    if toggles.deepl_api {
        match config::official_service() {
            Ok(service) => services.push(Box::new(service)),
            Err(error) => services.push(Box::new(UnavailableOfficial(error))),
        }
    }
    if ai.enabled {
        if let Some(error) = ai.error.clone().or_else(|| {
            (ai.base_url.trim().is_empty() || ai.model.trim().is_empty())
                .then(|| "请在设置中补全 AI 地址和模型".into())
        }) {
            services.push(Box::new(UnavailableAi(error)));
        } else {
            services.push(Box::new(OpenAiCompat::new(
                ai.base_url.clone(),
                ai.api_key.clone(),
                ai.model.clone(),
            )));
        }
    }
    services
}
struct UnavailableAi(String);
struct UnavailableOfficial(String);
impl TranslateService for UnavailableOfficial {
    fn name(&self) -> &'static str {
        "DeepLApi"
    }
    fn translate(
        &self,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<Vec<String>, lucas_core::services::ServiceError> {
        Err(lucas_core::services::ServiceError::Configuration(
            self.0.clone(),
        ))
    }
}
impl TranslateService for UnavailableAi {
    fn name(&self) -> &'static str {
        "AI"
    }
    fn translate(
        &self,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<Vec<String>, lucas_core::services::ServiceError> {
        Err(lucas_core::services::ServiceError::Configuration(
            self.0.clone(),
        ))
    }
}
