resource "aws_autoscaling_group" "example" {
  name = "example"

  launch_template_overrides = [
    {
      instance_type = "c7g.xlarge"
    },
    {
      instance_type = "m7g.xlarge"
    },
    {
      instance_type = "c7gd.xlarge"
    },
  ]
}
