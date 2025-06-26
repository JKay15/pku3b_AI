from pydantic import BaseModel

class download_document_Input(BaseModel):
    course: str
    title: str
