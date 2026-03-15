// Example native script entrypoint for a future nu C++ scripting bridge.
// The current editor/runtime stores this attachment and uses Play mode to
// drive the object with a built-in player controller path.

class CarController {
public:
    void OnStart() {
        PlayerCamera::Attach(this);
    }

    void OnUpdate(float deltaTime) {
        if (Input::KeyDown(Key::W)) MoveForward(4.5f * deltaTime);
        if (Input::KeyDown(Key::S)) MoveBackward(4.5f * deltaTime);
        if (Input::KeyDown(Key::A)) MoveLeft(4.5f * deltaTime);
        if (Input::KeyDown(Key::D)) MoveRight(4.5f * deltaTime);
    }
};
