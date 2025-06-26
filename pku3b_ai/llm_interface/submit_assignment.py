from pydantic import BaseModel

class submit_assignment_Input(BaseModel):
    course: str
    title: str
    filepath: str
