def build_index(documents):
    """Compile documents into an in-memory index."""
    return {doc.id: doc for doc in documents}


def retrieve(index, query):
    """Return documents whose text contains the exact query."""
    return [doc for doc in index.values() if query in doc.text]


class Retriever:
    def __init__(self, index):
        self.index = index
