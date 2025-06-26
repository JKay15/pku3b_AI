from pydantic import BaseModel

class list_assignments_Input(BaseModel):
    course: int
