// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

contract RoleManager {
    enum Role {
        None,
        Admin,
        Viewer
    }

    mapping(address => Role) private _roles;
    address public owner;

    event RoleUpdated(address indexed user, Role previousRole, Role newRole);
    event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);

    constructor() {
        owner = msg.sender;
        _roles[msg.sender] = Role.Admin;
        emit OwnershipTransferred(address(0), msg.sender);
        emit RoleUpdated(msg.sender, Role.None, Role.Admin);
    }

    modifier onlyOwner() {
        require(msg.sender == owner, "RoleManager: not owner");
        _;
    }

    modifier onlyRoleManager() {
        require(
            msg.sender == owner || _roles[msg.sender] == Role.Admin,
            "RoleManager: insufficient permissions"
        );
        _;
    }

    function transferOwnership(address newOwner) external onlyOwner {
        require(newOwner != address(0), "RoleManager: zero owner");
        address previousOwner = owner;
        owner = newOwner;
        _roles[newOwner] = Role.Admin;
        emit OwnershipTransferred(previousOwner, newOwner);
        emit RoleUpdated(newOwner, Role.None, Role.Admin);
    }

    function renounceOwnership() external onlyOwner {
        address previousOwner = owner;
        owner = address(0);
        emit OwnershipTransferred(previousOwner, address(0));
    }

    function setRole(address user, Role role) external onlyRoleManager {
        require(user != address(0), "RoleManager: zero user");
        Role previousRole = _roles[user];
        _roles[user] = role;
        emit RoleUpdated(user, previousRole, role);
    }

    function getRole(address user) external view returns (Role) {
        return _roles[user];
    }
}
