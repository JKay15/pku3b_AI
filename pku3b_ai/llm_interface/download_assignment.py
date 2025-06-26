from pydantic import BaseModel

class download_assignment_Input(BaseModel):
    course: str
    title: str
