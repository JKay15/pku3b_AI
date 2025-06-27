from pydantic import BaseModel

class list_documents_Input(BaseModel):
    course: int
