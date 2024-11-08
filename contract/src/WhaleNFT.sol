// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

contract WhaleNFT {
    

    function setNumber(uint256 newNumber) public {
        number = newNumber;
    }

    function increment() public {
        number++;
    }
}
