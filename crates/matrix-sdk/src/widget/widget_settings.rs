use language_tags::LanguageTag;
use ruma::api::client::profile::get_profile;
use url::{form_urlencoded::Serializer, Url, UrlQuery};

use crate::Room;

mod url_props {
    use url::Url;
    use urlencoding::encode;

    pub struct QueryProperties {
        pub(crate) widget_id: String,
        pub(crate) avatar_url: String,
        pub(crate) display_name: String,
        pub(crate) user_id: String,
        pub(crate) room_id: String,
        pub(crate) language: String,
        pub(crate) client_theme: String,
        pub(crate) client_id: String,
        pub(crate) device_id: String,
        pub(crate) homeserver_url: String,
    }

    pub fn replace_properties(url: &mut Url, props: QueryProperties) {
        *url = Url::parse(
            &url.as_str()
                .replace(WIDGET_ID.placeholder, &encode(&props.widget_id))
                .replace(AVATAR_URL.placeholder, &encode(&props.avatar_url))
                .replace(DEVICE_ID.placeholder, &encode(&props.device_id))
                .replace(DISPLAY_NAME.placeholder, &encode(&props.display_name))
                .replace(HOMESERVER_URL.placeholder, &encode(&props.homeserver_url))
                .replace(USER_ID.placeholder, &encode(&props.user_id))
                .replace(ROOM_ID.placeholder, &encode(&props.room_id))
                .replace(LANGUAGE.placeholder, &encode(&props.language))
                .replace(CLIENT_THEME.placeholder, &encode(&props.client_theme))
                .replace(CLIENT_ID.placeholder, &encode(&props.client_id)),
        )
        .unwrap();
    }

    pub struct Property {
        pub name: &'static str,
        pub placeholder: &'static str,
    }

    pub static USER_ID: Property = Property { name: "userId", placeholder: "$matrix_user_id" };
    pub static ROOM_ID: Property = Property { name: "roomId", placeholder: "$matrix_room_id" };
    pub static WIDGET_ID: Property =
        Property { name: "widgetId", placeholder: "$matrix_widget_id" };
    pub static AVATAR_URL: Property =
        Property { name: "avatarUrl", placeholder: "$matrix_avatar_url" };
    pub static DISPLAY_NAME: Property =
        Property { name: "displayname", placeholder: "$matrix_display_name" };
    pub static LANGUAGE: Property =
        Property { name: "lang", placeholder: "$org.matrix.msc2873.client_language" };
    pub static CLIENT_THEME: Property =
        Property { name: "theme", placeholder: "$org.matrix.msc2873.client_theme" };
    pub static CLIENT_ID: Property =
        Property { name: "clientId", placeholder: "$org.matrix.msc2873.client_id" };
    pub static DEVICE_ID: Property =
        Property { name: "deviceId", placeholder: "$org.matrix.msc2873.matrix_device_id" };
    pub static HOMESERVER_URL: Property =
        Property { name: "baseUrl", placeholder: "$org.matrix.msc4039.matrix_base_url" };
}
/// Settings of the widget.
#[derive(Debug, Clone)]
pub struct WidgetSettings {
    id: String,

    init_after_content_load: bool,

    raw_url: Url,
}

impl WidgetSettings {
    /// Widget's unique identifier.
    pub fn id(&self) -> &String {
        &self.id
    }

    /// Whether or not the widget should be initialized on load message
    /// (`ContentLoad` message), or upon creation/attaching of the widget to
    /// the SDK's state machine that drives the API.

    pub fn init_after_content_load(&self) -> bool {
        self.init_after_content_load
    }

    /// This contains the url from the widget state event.
    /// In this url placeholders can be used to pass information from the client
    /// to the widget. Possible values are: `$matrix_widget_id`,
    /// `$matrix_display_name`...
    ///
    /// # Examples
    ///
    /// e.g `http://widget.domain?username=$userId`
    /// will become: `http://widget.domain?username=@user_matrix_id:server.domain`.
    pub fn raw_url(&self) -> &Url {
        &self.raw_url
    }

    /// Get the base url of the widget. Used as the target for PostMessages. In
    /// case the widget is in a webview and not an IFrame. It contains the schema and the authority e.g. `https://my.domain.org`
    /// A postmessage would be send using: `postmessage(myMessage,
    /// widget_base_url)`
    pub fn base_url(&self) -> Option<Url> {
        base_url(&self.raw_url)
    }
    /// Create the actual Url that can be used to setup the WebView or IFrame
    /// that contains the widget.
    ///
    /// # Arguments
    ///
    /// * `room` - A matrix room which is used to query the logged in username
    /// * `props` - Properties from the client that can be used by a widget to
    ///   adapt to the client. e.g. language, font-scale...
    pub async fn generate_webview_url(
        &self,
        room: &Room,
        props: ClientProperties,
    ) -> Result<Url, url::ParseError> {
        let empty_profile = get_profile::v3::Response::new(None, None);
        self._generate_webview_url(
            room.client().account().get_profile().await.unwrap_or(empty_profile),
            room.own_user_id().to_string(),
            room.room_id().to_string(),
            room.client().device_id().map(|d| d.to_string()).unwrap_or("".to_owned()),
            room.client().homeserver().await.to_string(),
            props,
        )
    }
    fn _generate_webview_url(
        &self,
        profile: get_profile::v3::Response,
        user_id: String,
        room_id: String,
        device_id: String,
        homeserver_url: String,
        client_props: ClientProperties,
    ) -> Result<Url, url::ParseError> {
        let avatar_url = profile.avatar_url.map(|url| url.to_string()).unwrap_or("".to_owned());

        let query_props = url_props::QueryProperties {
            widget_id: self.id.clone(),
            avatar_url,
            display_name: profile.displayname.unwrap_or("".to_owned()),
            user_id,
            room_id,
            language: client_props.language.to_string(),
            client_theme: client_props.theme,
            client_id: client_props.client_id,
            device_id,
            homeserver_url,
        };
        let mut generated_url = self.raw_url.clone();
        url_props::replace_properties(&mut generated_url, query_props);

        Ok(generated_url)
    }
    /// `WidgetSettings` are usually created from a state event.
    /// (currently unimplemented)
    /// But in some cases the client wants to create custom `WidgetSettings`
    /// for specific rooms based on other conditions.
    /// This function returns a `WidgetSettings` object which can be used
    /// to setup a widget using `run_client_widget_api`
    /// and to generate the correct url for the widget.
    ///
    /// # Arguments
    /// * `element_call_url` - the url to the app e.g. https://call.element.io, https://call.element.dev
    /// * `id` - the widget id.
    /// * `parentUrl` - The url that is used as the target for the PostMessages
    ///   sent by the widget (to the client). For a web app client this is the
    ///   client url. In case of using other platforms the client most likely is
    ///   setup up to listen to postmessages in the same webview the widget is
    ///   hosted. In this case the parent_url is set to the url of the webview
    ///   with the widget. Be aware, that this means, the widget will receive
    ///   its own postmessage messages. The matrix-widget-api (js) ignores those
    ///   so this works but it might break custom implementations. So always
    ///   keep this in mind. Defaults to `element_call_url` for the non IFrame
    ///   (dedicated webview) usecase.
    /// * `hide_header` - defines if the branding header of Element call should
    ///   be hidden. (default: `true`)
    /// * `preload` - if set, the lobby will be skipped and the widget will join
    ///   the call on the `io.element.join` action. (default: `false`)
    /// * `font_scale` - The font scale which will be used inside element call.
    ///   (default: `1`)
    /// * `app_prompt` - whether element call should prompt the user to open in
    ///   the browser or the app (default: `false`).
    /// * `skip_lobby` Don't show the lobby and join the call immediately.
    ///   (default: `false`)
    /// * `confine_to_room` Make it not possible to get to the calls list in the
    ///   webview. (default: `true`)
    /// * `fonts` A list of fonts to adapt to ios/android system fonts.
    ///   (default: `[]`)
    /// * `analytics_id` - Can be used to pass a PostHog id to element call.
    pub fn new_virtual_element_call_widget(
        element_call_url: String,
        widget_id: String,
        parent_url: Option<String>,
        hide_header: Option<bool>,
        preload: Option<bool>,
        font_scale: Option<f64>,
        app_prompt: Option<bool>,
        skip_lobby: Option<bool>,
        confine_to_room: Option<bool>,
        fonts: Option<Vec<String>>,
        analytics_id: Option<String>,
    ) -> Result<Self, url::ParseError> {
        fn append_property(query: &mut Serializer<'_, UrlQuery<'_>>, prop: &url_props::Property) {
            query.append_pair(prop.name, prop.placeholder);
        }
        let mut raw_url: Url = Url::parse(&format!("{element_call_url}/room"))?;

        // ----- ALL THIS GETS MOVED INTO THE FRAGMENT
        {
            let mut query = raw_url.query_pairs_mut();

            // Default widget url template parameters:
            append_property(&mut query, &url_props::LANGUAGE);
            append_property(&mut query, &url_props::CLIENT_THEME);
        }

        {
            let mut query = raw_url.query_pairs_mut();

            // Custom element call url parameters:
            if app_prompt.unwrap_or(false) {
                query.append_pair("embed", "true");
            }
            query.append_pair("hideHeader", &hide_header.unwrap_or(true).to_string());
            query.append_pair("preload", &preload.unwrap_or(false).to_string());
            if let Some(analytics_id) = analytics_id {
                query.append_pair("analyticsID", &analytics_id);
            }
            if let Some(scale) = font_scale {
                query.append_pair("fontScale", &scale.to_string());
            }
            query.append_pair("skipLobby", &skip_lobby.unwrap_or(false).to_string());
            query.append_pair("confineToRoom", &confine_to_room.unwrap_or(true).to_string());
            if let Some(fonts) = fonts {
                query.append_pair("fonts", &fonts.join(","));
            }
        }

        // Transform the url to a have all the params inside the fragment (to keep the
        // traffic to the server minimal and most importantly don't send the passwords)
        if let Some(query) = raw_url.clone().query() {
            raw_url.set_query(None);
            raw_url.set_fragment(Some(&format!("?{}", query)));
        }
        // ----- ALL THIS Becomes part of the query

        {
            // We want those to be before the fragment (#)
            let mut query = raw_url.query_pairs_mut();

            // Default widget url template parameters:
            query.append_pair("parentUrl", &parent_url.unwrap_or(element_call_url));
            append_property(&mut query, &url_props::WIDGET_ID);
            append_property(&mut query, &url_props::USER_ID);
            append_property(&mut query, &url_props::DEVICE_ID);
            append_property(&mut query, &url_props::ROOM_ID);
            append_property(&mut query, &url_props::HOMESERVER_URL);
        }

        // Revert the encoding for the template parameters. So we can have a unified
        // replace logic.
        let raw_url = Url::parse(&raw_url.as_str().replace("%24", "$"))?;

        // for EC we always want init on content load to be true.
        Ok(Self { id: widget_id, init_after_content_load: true, raw_url })
    }

    /// Create a new WidgetSettings instance
    pub fn new(
        id: String,
        init_after_content_load: bool,
        raw_url: &str,
    ) -> Result<Self, url::ParseError> {
        Ok(Self { id, init_after_content_load, raw_url: Url::parse(raw_url)? })
    }
    // TODO: add From<WidgetStateEvent> so that WidgetSetting can be build
    // by using the room state directly:
    // Something like: room.get_widgets() -> Vec<WidgetStateEvent>
}

/// The set of settings and properties for the widget based on the client
/// configuration. Those values are used generate the widget url.
#[derive(Debug)]
pub struct ClientProperties {
    /// The client_id provides the widget with the option to behave differently
    /// for different clients. e.g org.example.ios.
    pub client_id: String,
    /// The language the client is set to e.g. en-us.
    pub language: LanguageTag,
    /// A string describing the theme (dark, light) or org.example.dark.
    pub theme: String,
}
impl ClientProperties {
    /// Create client Properties with a String as the language_tag.
    /// If a malformatted language_tag is provided it will default to en-US.
    /// # Arguments
    /// * `client_id` the client identifier. This allows widgets to adapt to
    ///   specific clients (e.g. `io.element.web`)
    /// * `language` the language that is used in the client. (default: `en-US`)
    /// * `theme` the theme (dark, light) or org.example.dark. (default:
    ///   `light`)
    pub fn new(client_id: &str, language: Option<String>, theme: Option<String>) -> Self {
        // its save to unwrap "en-us"
        let default_language = LanguageTag::parse(&"en-us").unwrap();
        let default_theme = "light".to_owned();
        ClientProperties {
            language: language
                .and_then(|l| LanguageTag::parse(&l).ok())
                .unwrap_or(default_language),
            client_id: client_id.to_owned(),
            theme: theme.unwrap_or(default_theme),
        }
    }
}

fn base_url(url: &Url) -> Option<Url> {
    let mut url = url.clone();
    match url.path_segments_mut() {
        Ok(mut path) => path.clear(),
        Err(_) => return None,
    };
    url.set_query(None);
    url.set_fragment(None);
    Some(url)
}

#[cfg(test)]
mod tests {
    use ruma::api::client::profile::get_profile;
    use url::Url;

    use super::{
        url_props::{replace_properties, QueryProperties},
        WidgetSettings,
    };
    use crate::widget::ClientProperties;

    const EXAMPLE_URL: &str = "https://my.widget.org/custom/path?\
    widgetId=$matrix_widget_id\
    &deviceId=$org.matrix.msc2873.matrix_device_id\
    &avatarUrl=$matrix_avatar_url\
    &displayname=$matrix_display_name\
    &lang=$org.matrix.msc2873.client_language\
    &theme=$org.matrix.msc2873.client_theme\
    &clientId=$org.matrix.msc2873.client_id\
    &baseUrl=$org.matrix.msc4039.matrix_base_url";

    const WIDGET_ID: &str = "1/@#w23";

    fn get_example_url() -> Url {
        Url::parse(EXAMPLE_URL).expect("EXAMPLE_URL is malformatted")
    }

    fn get_example_props() -> QueryProperties {
        QueryProperties {
            widget_id: String::from("!@/abc_widget_id"),
            avatar_url: "!@/abc_avatar_url".to_owned(),
            display_name: "!@/abc_display_name".to_owned(),
            user_id: "!@/abc_user_id".to_owned(),
            room_id: "!@/abc_room_id".to_owned(),
            language: "!@/abc_language".to_owned(),
            client_theme: "!@/abc_client_theme".to_owned(),
            client_id: "!@/abc_client_id".to_owned(),
            device_id: "!@/abc_device_id".to_owned(),
            homeserver_url: "!@/abc_base_url".to_owned(),
        }
    }
    fn get_widget_settings() -> WidgetSettings {
        WidgetSettings::new_virtual_element_call_widget(
            "https://call.element.io".to_owned(),
            WIDGET_ID.to_owned(),
            None,
            Some(true),
            Some(true),
            None,
            Some(true),
            Some(false),
            Some(true),
            None,
            None,
        )
        .expect("could not parse virtual element call widget")
    }
    #[test]
    fn replace_all_properties() {
        let mut url = get_example_url();
        const CONVERTED_URL: &str = "https://my.widget.org/custom/path?widgetId=%21%40%2Fabc_widget_id&deviceId=%21%40%2Fabc_device_id&avatarUrl=%21%40%2Fabc_avatar_url&displayname=%21%40%2Fabc_display_name&lang=%21%40%2Fabc_language&theme=%21%40%2Fabc_client_theme&clientId=%21%40%2Fabc_client_id&baseUrl=%21%40%2Fabc_base_url";
        replace_properties(&mut url, get_example_props());
        assert_eq!(url.as_str(), CONVERTED_URL);
    }

    #[test]
    fn new_virtual_element_call_widget_base_url() {
        let widget_settings = get_widget_settings();
        assert_eq!(widget_settings.base_url().unwrap().as_str(), "https://call.element.io/");
    }
    #[test]
    fn new_virtual_element_call_widget_raw_url() {
        assert_eq!(get_widget_settings().raw_url().as_str(), "https://call.element.io/room?parentUrl=https%3A%2F%2Fcall.element.io&widgetId=%24matrix_widget_id#?userId=$matrix_user_id&deviceId=$org.matrix.msc2873.matrix_device_id&roomId=$matrix_room_id&lang=$org.matrix.msc2873.client_language&theme=$org.matrix.msc2873.client_theme&baseUrl=$org.matrix.msc4039.matrix_base_url&embed=true&hideHeader=true&preload=true&skipLobby=false&confineToRoom=true");
    }
    #[test]
    fn new_virtual_element_call_widget_id() {
        assert_eq!(get_widget_settings().id(), WIDGET_ID);
    }
    #[test]
    fn new_virtual_element_call_widget_webview_url() {
        let gen = get_widget_settings()
            ._generate_webview_url(
                get_profile::v3::Response::new(None, None),
                "@test:user.org".to_string(),
                "!room_id:room.org".to_string(),
                "ABCDEFG".to_string(),
                "https://client-matrix.server.org".to_string(),
                ClientProperties {
                    client_id: "io.my_matrix.client".to_string(),
                    language: language_tags::LanguageTag::parse("en-us").unwrap(),
                    theme: "light".to_string(),
                },
            )
            .unwrap()
            .to_string();
        assert_eq!(
            &gen,
            "https://call.element.io/room?\
            parentUrl=https%3A%2F%2Fcall.element.io&widgetId=%24matrix_widget_id\
            #?userId=%40test%3Auser.org&deviceId=ABCDEFG&roomId=%21room_id%3Aroom.org\
            &lang=en-US&theme=light\
            &baseUrl=https%3A%2F%2Fclient-matrix.server.org&embed=true&hideHeader=true\
            &preload=true&skipLobby=false&confineToRoom=true"
        );
    }
}
