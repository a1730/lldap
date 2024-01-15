use crate::{
    components::router::AppRoute,
    infra::{
        api::HostService,
        common_component::{CommonComponent, CommonComponentParts},
    },
};
use anyhow::{bail, Result};
use gloo_console::log;
use graphql_client::GraphQLQuery;
use lldap_auth::{opaque, registration};
use validator_derive::Validate;
use yew::prelude::*;
use yew_form_derive::Model;
use yew_router::{prelude::History, scope_ext::RouterScopeExt};

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "../schema.graphql",
    query_path = "queries/create_user.graphql",
    response_derives = "Debug",
    custom_scalars_module = "crate::infra::graphql"
)]
pub struct CreateUser;

pub struct CreateUserForm {
    common: CommonComponentParts<Self>,
    form: yew_form::Form<CreateUserModel>,
}

#[derive(Model, Validate, PartialEq, Eq, Clone, Default)]
pub struct CreateUserModel {
    #[validate(length(min = 1, message = "Username is required"))]
    username: String,
    #[validate(email(message = "A valid email is required"))]
    email: String,
    display_name: String,
    first_name: String,
    last_name: String,
    #[validate(custom(
        function = "empty_or_long",
        message = "Password should be longer than 8 characters (or left empty)"
    ))]
    password: String,
    #[validate(must_match(other = "password", message = "Passwords must match"))]
    confirm_password: String,
}

fn empty_or_long(value: &str) -> Result<(), validator::ValidationError> {
    if value.is_empty() || value.len() >= 8 {
        Ok(())
    } else {
        Err(validator::ValidationError::new(""))
    }
}

pub enum Msg {
    Update,
    SubmitForm,
    CreateUserResponse(Result<create_user::ResponseData>),
    SuccessfulCreation,
    RegistrationStartResponse(
        (
            opaque::client::registration::ClientRegistration,
            Result<Box<registration::ServerRegistrationStartResponse>>,
        ),
    ),
    RegistrationFinishResponse(Result<()>),
}

impl CommonComponent<CreateUserForm> for CreateUserForm {
    fn handle_msg(
        &mut self,
        ctx: &Context<Self>,
        msg: <Self as Component>::Message,
    ) -> Result<bool> {
        match msg {
            Msg::Update => Ok(true),
            Msg::SubmitForm => {
                if !self.form.validate() {
                    bail!("Check the form for errors");
                }
                let model = self.form.model();
                let to_option = |s: String| if s.is_empty() { None } else { Some(s) };
                let req = create_user::Variables {
                    user: create_user::CreateUserInput {
                        id: model.username,
                        email: model.email,
                        displayName: to_option(model.display_name),
                        firstName: to_option(model.first_name),
                        lastName: to_option(model.last_name),
                        avatar: None,
                        attributes: None,
                    },
                };
                self.common.call_graphql::<CreateUser, _>(
                    ctx,
                    req,
                    Msg::CreateUserResponse,
                    "Error trying to create user",
                );
                Ok(true)
            }
            Msg::CreateUserResponse(r) => {
                match r {
                    Err(e) => return Err(e),
                    Ok(r) => log!(&format!(
                        "Created user '{}' at '{}'",
                        &r.create_user.id, &r.create_user.creation_date
                    )),
                };
                let model = self.form.model();
                let user_id = model.username;
                let password = model.password;
                if !password.is_empty() {
                    // User was successfully created, let's register the password.
                    let mut rng = rand::rngs::OsRng;
                    let opaque::client::registration::ClientRegistrationStartResult {
                        state,
                        message,
                    } = opaque::client::registration::start_registration(
                        password.as_bytes(),
                        &mut rng,
                    )?;
                    let req = registration::ClientRegistrationStartRequest {
                        username: user_id.into(),
                        registration_start_request: message,
                    };
                    self.common
                        .call_backend(ctx, HostService::register_start(req), move |r| {
                            Msg::RegistrationStartResponse((state, r))
                        });
                } else {
                    self.update(ctx, Msg::SuccessfulCreation);
                }
                Ok(false)
            }
            Msg::RegistrationStartResponse((registration_start, response)) => {
                let response = response?;
                let mut rng = rand::rngs::OsRng;
                let registration_upload = opaque::client::registration::finish_registration(
                    registration_start,
                    response.registration_response,
                    &mut rng,
                )?;
                let req = registration::ClientRegistrationFinishRequest {
                    server_data: response.server_data,
                    registration_upload: registration_upload.message,
                };
                self.common.call_backend(
                    ctx,
                    HostService::register_finish(req),
                    Msg::RegistrationFinishResponse,
                );
                Ok(false)
            }
            Msg::RegistrationFinishResponse(response) => {
                response?;
                self.handle_msg(ctx, Msg::SuccessfulCreation)
            }
            Msg::SuccessfulCreation => {
                ctx.link().history().unwrap().push(AppRoute::ListUsers);
                Ok(true)
            }
        }
    }

    fn mut_common(&mut self) -> &mut CommonComponentParts<Self> {
        &mut self.common
    }
}

impl Component for CreateUserForm {
    type Message = Msg;
    type Properties = ();

    fn create(_: &Context<Self>) -> Self {
        Self {
            common: CommonComponentParts::<Self>::create(),
            form: yew_form::Form::<CreateUserModel>::new(CreateUserModel::default()),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        CommonComponentParts::<Self>::update(self, ctx, msg)
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = &ctx.link();
        type Field = yew_form::Field<CreateUserModel>;
        html! {
          <div class="row justify-content-center">
            <form class="form py-3" style="max-width: 636px">
              <div class="row mb-3">
                <h5 class="fw-bold">{"Create a user"}</h5>
              </div>
              <div class="form-group row mb-3">
                <label for="username"
                  class="form-label col-4 col-form-label">
                  {"User name"}
                  <span class="text-danger">{"*"}</span>
                  {":"}
                </label>
                <div class="col-8">
                  <Field
                    form={&self.form}
                    field_name="username"
                    class="form-control"
                    class_invalid="is-invalid has-error"
                    class_valid="has-success"
                    autocomplete="username"
                    oninput={link.callback(|_| Msg::Update)} />
                  <div class="invalid-feedback">
                    {&self.form.field_message("username")}
                  </div>
                </div>
              </div>
              <div class="form-group row mb-3">
                <label for="email"
                  class="form-label col-4 col-form-label">
                  {"Email"}
                  <span class="text-danger">{"*"}</span>
                  {":"}
                </label>
                <div class="col-8">
                  <Field
                    form={&self.form}
                    input_type="email"
                    field_name="email"
                    class="form-control"
                    class_invalid="is-invalid has-error"
                    class_valid="has-success"
                    autocomplete="email"
                    oninput={link.callback(|_| Msg::Update)} />
                  <div class="invalid-feedback">
                    {&self.form.field_message("email")}
                  </div>
                </div>
              </div>
              <div class="form-group row mb-3">
                <label for="display_name"
                  class="form-label col-4 col-form-label">
                  {"Display name:"}
                </label>
                <div class="col-8">
                  <Field
                    form={&self.form}
                    autocomplete="name"
                    class="form-control"
                    class_invalid="is-invalid has-error"
                    class_valid="has-success"
                    field_name="display_name"
                    oninput={link.callback(|_| Msg::Update)} />
                  <div class="invalid-feedback">
                    {&self.form.field_message("display_name")}
                  </div>
                </div>
              </div>
              <div class="form-group row mb-3">
                <label for="first_name"
                  class="form-label col-4 col-form-label">
                  {"First name:"}
                </label>
                <div class="col-8">
                  <Field
                    form={&self.form}
                    autocomplete="given-name"
                    class="form-control"
                    class_invalid="is-invalid has-error"
                    class_valid="has-success"
                    field_name="first_name"
                    oninput={link.callback(|_| Msg::Update)} />
                  <div class="invalid-feedback">
                    {&self.form.field_message("first_name")}
                  </div>
                </div>
              </div>
              <div class="form-group row mb-3">
                <label for="last_name"
                  class="form-label col-4 col-form-label">
                  {"Last name:"}
                </label>
                <div class="col-8">
                  <Field
                    form={&self.form}
                    autocomplete="family-name"
                    class="form-control"
                    class_invalid="is-invalid has-error"
                    class_valid="has-success"
                    field_name="last_name"
                    oninput={link.callback(|_| Msg::Update)} />
                  <div class="invalid-feedback">
                    {&self.form.field_message("last_name")}
                  </div>
                </div>
              </div>
              <div class="form-group row mb-3">
                <label for="password"
                  class="form-label col-4 col-form-label">
                  {"Password:"}
                </label>
                <div class="col-8">
                  <Field
                    form={&self.form}
                    input_type="password"
                    field_name="password"
                    class="form-control"
                    class_invalid="is-invalid has-error"
                    class_valid="has-success"
                    autocomplete="new-password"
                    oninput={link.callback(|_| Msg::Update)} />
                  <div class="invalid-feedback">
                    {&self.form.field_message("password")}
                  </div>
                </div>
              </div>
              <div class="form-group row mb-3">
                <label for="confirm_password"
                  class="form-label col-4 col-form-label">
                  {"Confirm password:"}
                </label>
                <div class="col-8">
                  <Field
                    form={&self.form}
                    input_type="password"
                    field_name="confirm_password"
                    class="form-control"
                    class_invalid="is-invalid has-error"
                    class_valid="has-success"
                    autocomplete="new-password"
                    oninput={link.callback(|_| Msg::Update)} />
                  <div class="invalid-feedback">
                    {&self.form.field_message("confirm_password")}
                  </div>
                </div>
              </div>
              <div class="form-group row justify-content-center">
                <button
                  class="btn btn-primary col-auto col-form-label mt-4"
                  disabled={self.common.is_task_running()}
                  type="submit"
                  onclick={link.callback(|e: MouseEvent| {e.prevent_default(); Msg::SubmitForm})}>
                  <i class="bi-save me-2"></i>
                  {"Submit"}
                </button>
              </div>
            </form>
            {
              if let Some(e) = &self.common.error {
                html! {
                  <div class="alert alert-danger">
                    {e.to_string() }
                  </div>
                }
              } else { html! {} }
            }
          </div>
        }
    }
}
