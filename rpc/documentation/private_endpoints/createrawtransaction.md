Creates a new transaction and returns the encoded transaction along with the encoded records.

### Protected Endpoint

Yes

### Arguments

|          Parameter         |  Type  | Required |                        Description                       |
|:-------------------------- |:------:|:--------:|:-------------------------------------------------------- |
| `old_records`              |  array |    Yes   | An array of hex encoded records to be spent              |
| `old_account_private_keys` |  array |    Yes   | An array of private keys authorized to spend the records |
| `recipients`               |  array |    Yes   | The array of transaction recipient objects               |
| `memo`                     | string |    No    | The transaction memo                                     |
| `network_id`               | number |    Yes   | The network id of the transaction                        |

Transaction Recipient Object

| Parameter |  Type  |            Description           |
|:---------:|:------:|:--------------------------------:|
| `address` | string | The recipient address            |
| `value`   | number | The amount sent to the recipient |

### Response

|       Parameter       |  Type  |                  Description                  |
|:---------------------:|:------:|:--------------------------------------------- |
| `encoded_transaction` | string | The hex encoding of the generated transaction |
| `encoded_records`     | array  | The hex encodings of the generated records    |

### Example
```ignore
curl --user username:password --data-binary '{ 
    "jsonrpc":"2.0",
    "id": "1",
    "method": "createrawtransaction",
    "params": [
       {
        "old_records": ["record_hexstring"],
        "old_account_private_keys": ["private_key_string"],
        "recipients": [{
                "address": "address_string",
                "amount": amount
        }],
        "memo": "memo_hexstring",
        "network_id": 0
       }
    ]
}' -H 'content-type: application/json' http://127.0.0.1:3030/
```
