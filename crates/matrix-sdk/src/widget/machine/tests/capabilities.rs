// Copyright 2023 The Matrix.org Foundation C.I.C.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use assert_matches::assert_matches;
use serde_json::{from_value, json};

use super::{parse_msg, WIDGET_ID};
use crate::widget::machine::{
    incoming::MatrixDriverResponse, Action, IncomingMessage, MatrixDriverRequestData, WidgetMachine,
};

#[test]
fn machine_can_negotiate_capabilities_immediately() {
    let (mut machine, mut actions_recv) = WidgetMachine::new(WIDGET_ID.to_owned(), false);

    // Ask widget to provide desired capabilities.
    {
        let action = actions_recv.try_recv().unwrap();
        let msg = assert_matches!(action, Action::SendToWidget(msg) => msg);
        let (msg, request_id) = parse_msg(&msg);
        assert_eq!(
            msg,
            json!({
                "api": "toWidget",
                "widgetId": WIDGET_ID,
                "action": "capabilities",
                "data": {},
            }),
        );

        machine.process(IncomingMessage::WidgetMessage(json_string!({
            "api": "toWidget",
            "widgetId": WIDGET_ID,
            "requestId": request_id,
            "action": "capabilities",
            "data": {},
            "response": ["org.matrix.msc2762.receive.state_event:m.room.member"],
        })));
    }

    // Try to acquire permissions by sending a request to a matrix driver.
    {
        let action = actions_recv.try_recv().unwrap();
        let (request_id, permissions) = assert_matches!(
            action,
            Action::MatrixDriverRequest {
                request_id,
                data: MatrixDriverRequestData::AcquirePermissions(data)
            } => (request_id, data.desired_permissions)
        );
        assert_eq!(
            permissions,
            from_value(json!(["org.matrix.msc2762.receive.state_event:m.room.member"])).unwrap()
        );

        let response = MatrixDriverResponse::PermissionsAcquired(permissions);
        machine
            .process(IncomingMessage::MatrixDriverResponse { request_id, response: Ok(response) });
    }

    // Inform the widget about the acquired permissions.
    {
        let action = actions_recv.try_recv().unwrap();
        let msg = assert_matches!(action, Action::SendToWidget(msg) => msg);
        let (msg, request_id) = parse_msg(&msg);
        assert_eq!(
            msg,
            json!({
                "api": "toWidget",
                "widgetId": WIDGET_ID,
                "action": "notify_capabilities",
                "data": {
                    "requested": ["org.matrix.msc2762.receive.state_event:m.room.member"],
                    "approved": ["org.matrix.msc2762.receive.state_event:m.room.member"],
                },
            }),
        );

        machine.process(IncomingMessage::WidgetMessage(json_string!({
            "api": "toWidget",
            "widgetId": WIDGET_ID,
            "requestId": request_id,
            "action": "notify_capabilities",
            "data": {
                "requested": ["org.matrix.msc2762.receive.state_event:m.room.member"],
                "approved": ["org.matrix.msc2762.receive.state_event:m.room.member"],
            },
            "response": {},
        })));
    }

    assert_matches!(actions_recv.try_recv(), Err(_));
}

#[test]
fn machine_can_request_capabilities_on_content_load() {
    let (_machine, mut actions_recv) = WidgetMachine::new(WIDGET_ID.to_owned(), true);
    assert_matches!(actions_recv.try_recv(), Err(_));

    // TODO: Do the actual content load dance
}
