class Observer {
  PVector position;
  PVector velocity;
  Quaternion rotation;

  Observer(PVector position, PVector velocity, Quaternion rotation) {
    this.position = position;
    this.velocity = velocity;
    this.rotation = rotation;
  }
}
